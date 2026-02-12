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

/// Executor for a Lambda function
pub struct FunctionExecutor {
    config: LambdaConfig,
    source_dir: std::path::PathBuf,
}

impl FunctionExecutor {
    pub fn new(config: LambdaConfig, source_dir: std::path::PathBuf) -> Self {
        Self { config, source_dir }
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
        
        let handler_path = self.source_dir.join(format!("{}.js", file));
        
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
        
        let handler_path = self.source_dir.join(format!("{}.py", file));
        
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
