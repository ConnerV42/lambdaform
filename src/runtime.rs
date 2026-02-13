//! Lambda runtime execution
//!
//! Spawns local processes to execute Lambda handlers.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::{LambdaConfig, Runtime};

/// Lambda event structure (simplified API Gateway proxy format)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LambdaEvent {
    pub http_method: String,
    pub path: String,
    pub path_parameters: Option<HashMap<String, String>>,
    pub query_string_parameters: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub is_base64_encoded: bool,
}

/// Lambda context structure
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LambdaContext {
    pub function_name: String,
    pub function_version: String,
    pub memory_limit_in_mb: u32,
    pub aws_request_id: String,
    pub invoked_function_arn: String,
}

/// Lambda response structure
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LambdaResponse {
    pub status_code: u16,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    #[serde(default)]
    pub is_base64_encoded: bool,
}

/// Execution result from runtime
#[derive(Debug, Deserialize)]
struct RuntimeResult {
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

/// Authorizer event sent to Lambda authorizer functions
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizerEvent {
    /// "TOKEN" or "REQUEST"
    #[serde(rename = "type")]
    pub auth_type: String,
    /// Authorization header value (for TOKEN authorizers)
    pub authorization_token: Option<String>,
    /// Method ARN
    pub method_arn: String,
    /// HTTP method
    pub http_method: String,
    /// Request path
    pub path: String,
    /// Request headers (for REQUEST authorizers)
    pub headers: Option<HashMap<String, String>>,
    /// Query string parameters (for REQUEST authorizers)
    pub query_string_parameters: Option<HashMap<String, String>>,
}

/// Result from authorizer Lambda execution
#[derive(Debug, Clone)]
pub struct AuthorizerResult {
    pub is_authorized: bool,
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// Executor for a Lambda function
pub struct FunctionExecutor {
    config: LambdaConfig,
    source_dir: std::path::PathBuf,
}

impl FunctionExecutor {
    pub fn new(config: LambdaConfig, source_dir: std::path::PathBuf) -> Self {
        Self { config, source_dir }
    }
    
    /// Invoke the Lambda function as an authorizer
    pub async fn invoke_authorizer(&self, event: AuthorizerEvent) -> Result<AuthorizerResult> {
        let context = LambdaContext {
            function_name: self.config.function_name.clone(),
            function_version: "$LATEST".to_string(),
            memory_limit_in_mb: self.config.memory_size,
            aws_request_id: format!("local-auth-{}", uuid_simple()),
            invoked_function_arn: format!(
                "arn:aws:lambda:local:000000000000:function:{}",
                self.config.function_name
            ),
        };
        
        let payload = serde_json::json!({
            "event": event,
            "context": context,
        });
        
        // Execute using the same runtime as regular invocation
        let raw_response = match &self.config.runtime {
            Runtime::Nodejs18 | Runtime::Nodejs20 => {
                self.invoke_nodejs_raw(&payload).await?
            }
            Runtime::Python310 | Runtime::Python311 | Runtime::Python312 => {
                self.invoke_python_raw(&payload).await?
            }
            _ => {
                anyhow::bail!("Unsupported runtime for authorizer: {:?}", self.config.runtime)
            }
        };
        
        // Parse authorizer response
        // V1 TOKEN/REQUEST authorizers return an IAM policy with "Allow" or "Deny"
        // V2 simple response: { "isAuthorized": true/false, "context": {...} }
        
        // Check for simple format first (v2 style)
        if let Some(is_auth) = raw_response.get("isAuthorized").and_then(|v| v.as_bool()) {
            return Ok(AuthorizerResult {
                is_authorized: is_auth,
                context: raw_response.get("context").and_then(|c| {
                    serde_json::from_value(c.clone()).ok()
                }),
            });
        }
        
        // Check for IAM policy format (v1 style)
        if let Some(policy) = raw_response.get("policyDocument") {
            let is_authorized = policy.get("Statement")
                .and_then(|s| s.as_array())
                .map(|stmts| stmts.iter().any(|stmt| {
                    stmt.get("Effect").and_then(|e| e.as_str()) == Some("Allow")
                }))
                .unwrap_or(false);
            
            return Ok(AuthorizerResult {
                is_authorized,
                context: raw_response.get("context").and_then(|c| {
                    serde_json::from_value(c.clone()).ok()
                }),
            });
        }
        
        // If response doesn't match known formats, deny
        tracing::warn!("Authorizer returned unrecognized format, denying: {:?}", raw_response);
        Ok(AuthorizerResult {
            is_authorized: false,
            context: None,
        })
    }
    
    /// Invoke and return raw JSON value (used by authorizer)
    async fn invoke_nodejs_raw(&self, payload: &serde_json::Value) -> Result<serde_json::Value> {
        let (file, func) = parse_handler(&self.config.handler)?;
        let handler_path = find_handler_file(&self.source_dir, file, "js")?;
        
        let bootstrap = format!(
            r#"
const handler = require('{}');
const handlerFn = handler['{}'];

process.stdin.once('data', async (data) => {{
    const {{ event, context }} = JSON.parse(data.toString());
    try {{
        const result = await handlerFn(event, context);
        console.log(JSON.stringify({{ success: true, result }}));
        process.exit(0);
    }} catch (error) {{
        console.log(JSON.stringify({{ success: false, error: error.message }}));
        process.exit(0);
    }}
}});
"#,
            handler_path.display(), func
        );
        
        let mut child = Command::new("node")
            .arg("-e").arg(&bootstrap)
            .current_dir(&self.source_dir)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .envs(&self.config.environment)
            .spawn().context("Failed to spawn Node.js process")?;
        
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(serde_json::to_string(payload)?.as_bytes()).await?;
        stdin.flush().await?;
        drop(stdin);
        
        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if let Ok(result) = serde_json::from_str::<RuntimeResult>(line) {
                if result.success {
                    return result.result.ok_or_else(|| anyhow::anyhow!("No result from authorizer"));
                } else {
                    anyhow::bail!("Authorizer error: {}", result.error.unwrap_or_default());
                }
            }
        }
        anyhow::bail!("No valid response from authorizer Lambda")
    }
    
    /// Invoke Python and return raw JSON value (used by authorizer)
    async fn invoke_python_raw(&self, payload: &serde_json::Value) -> Result<serde_json::Value> {
        let (file, func) = parse_handler(&self.config.handler)?;
        let handler_path = find_handler_file(&self.source_dir, file, "py")?;
        
        let bootstrap = format!(
            r#"
import sys
import json
import importlib.util

spec = importlib.util.spec_from_file_location("handler", "{}")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
handler_fn = getattr(module, "{}")

payload = json.loads(sys.stdin.read())
try:
    result = handler_fn(payload["event"], payload["context"])
    print(json.dumps({{"success": True, "result": result}}))
except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
            handler_path.display(), func
        );
        
        let mut child = Command::new("python3")
            .arg("-c").arg(&bootstrap)
            .current_dir(&self.source_dir)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .envs(&self.config.environment)
            .spawn().context("Failed to spawn Python process")?;
        
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(serde_json::to_string(payload)?.as_bytes()).await?;
        stdin.flush().await?;
        drop(stdin);
        
        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if let Ok(result) = serde_json::from_str::<RuntimeResult>(line) {
                if result.success {
                    return result.result.ok_or_else(|| anyhow::anyhow!("No result from authorizer"));
                } else {
                    anyhow::bail!("Authorizer error: {}", result.error.unwrap_or_default());
                }
            }
        }
        anyhow::bail!("No valid response from authorizer Lambda")
    }
    
    /// Invoke the Lambda function with an event
    pub async fn invoke(&self, event: LambdaEvent) -> Result<LambdaResponse> {
        let context = LambdaContext {
            function_name: self.config.function_name.clone(),
            function_version: "$LATEST".to_string(),
            memory_limit_in_mb: self.config.memory_size,
            aws_request_id: format!("local-{}", uuid_simple()),
            invoked_function_arn: format!(
                "arn:aws:lambda:local:000000000000:function:{}",
                self.config.function_name
            ),
        };
        
        let payload = serde_json::json!({
            "event": event,
            "context": context,
        });
        
        match &self.config.runtime {
            Runtime::Nodejs18 | Runtime::Nodejs20 => {
                self.invoke_nodejs(&payload).await
            }
            Runtime::Python310 | Runtime::Python311 | Runtime::Python312 => {
                self.invoke_python(&payload).await
            }
            _ => {
                anyhow::bail!("Unsupported runtime: {:?}", self.config.runtime)
            }
        }
    }
    
    /// Invoke Node.js function
    async fn invoke_nodejs(&self, payload: &serde_json::Value) -> Result<LambdaResponse> {
        // Parse handler (e.g., "index.handler" -> file: index.js, function: handler)
        let (file, func) = parse_handler(&self.config.handler)?;
        
        let handler_path = find_handler_file(&self.source_dir, file, "js")?;
        
        // Node.js bootstrap script (inline)
        let bootstrap = format!(
            r#"
const handler = require('{}');
const handlerFn = handler['{}'];

process.stdin.once('data', async (data) => {{
    const {{ event, context }} = JSON.parse(data.toString());
    try {{
        const result = await handlerFn(event, context);
        console.log(JSON.stringify({{ success: true, result }}));
        process.exit(0);
    }} catch (error) {{
        console.log(JSON.stringify({{ success: false, error: error.message }}));
        process.exit(0);
    }}
}});
"#,
            handler_path.display(),
            func
        );
        
        let mut child = Command::new("node")
            .arg("-e")
            .arg(&bootstrap)
            .current_dir(&self.source_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&self.config.environment)
            .spawn()
            .context("Failed to spawn Node.js process")?;
        
        // Send payload
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(serde_json::to_string(payload)?.as_bytes())
            .await?;
        stdin.flush().await?;
        drop(stdin);
        
        // Read response
        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Find JSON line in output
        for line in stdout.lines() {
            if let Ok(result) = serde_json::from_str::<RuntimeResult>(line) {
                if result.success {
                    if let Some(value) = result.result {
                        let response: LambdaResponse = serde_json::from_value(value)?;
                        return Ok(response);
                    }
                } else {
                    anyhow::bail!("Lambda error: {}", result.error.unwrap_or_default());
                }
            }
        }
        
        anyhow::bail!("No valid response from Lambda")
    }
    
    /// Invoke Python function
    async fn invoke_python(&self, payload: &serde_json::Value) -> Result<LambdaResponse> {
        let (file, func) = parse_handler(&self.config.handler)?;
        
        let handler_path = find_handler_file(&self.source_dir, file, "py")?;
        
        let bootstrap = format!(
            r#"
import sys
import json
import importlib.util

spec = importlib.util.spec_from_file_location("handler", "{}")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
handler_fn = getattr(module, "{}")

payload = json.loads(sys.stdin.read())
try:
    result = handler_fn(payload["event"], payload["context"])
    print(json.dumps({{"success": True, "result": result}}))
except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
            handler_path.display(),
            func
        );
        
        let mut child = Command::new("python3")
            .arg("-c")
            .arg(&bootstrap)
            .current_dir(&self.source_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&self.config.environment)
            .spawn()
            .context("Failed to spawn Python process")?;
        
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(serde_json::to_string(payload)?.as_bytes())
            .await?;
        stdin.flush().await?;
        drop(stdin);
        
        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if let Ok(result) = serde_json::from_str::<RuntimeResult>(line) {
                if result.success {
                    if let Some(value) = result.result {
                        let response: LambdaResponse = serde_json::from_value(value)?;
                        return Ok(response);
                    }
                } else {
                    anyhow::bail!("Lambda error: {}", result.error.unwrap_or_default());
                }
            }
        }
        
        anyhow::bail!("No valid response from Lambda")
    }
}

/// Find handler file by searching common locations. Returns canonical (absolute) path.
fn find_handler_file(source_dir: &std::path::Path, file: &str, ext: &str) -> Result<std::path::PathBuf> {
    let filename = format!("{}.{}", file, ext);
    
    // Search order: root, src/, lib/, lambda/
    let candidates = [
        source_dir.join(&filename),
        source_dir.join("src").join(&filename),
        source_dir.join("lib").join(&filename),
        source_dir.join("lambda").join(&filename),
    ];
    
    for path in &candidates {
        if path.exists() {
            let canonical = path.canonicalize()
                .with_context(|| format!("Failed to canonicalize {}", path.display()))?;
            tracing::info!("Found handler at: {}", canonical.display());
            return Ok(canonical);
        }
    }
    
    anyhow::bail!(
        "Handler file '{}' not found. Searched: {}",
        filename,
        candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
    )
}

/// Parse handler string (e.g., "index.handler" -> ("index", "handler"))
fn parse_handler(handler: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = handler.rsplitn(2, '.').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid handler format: {}", handler);
    }
    Ok((parts[1], parts[0]))
}

/// Generate a simple UUID-like string
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    format!("{:x}-{:x}", duration.as_secs(), duration.subsec_nanos())
}
