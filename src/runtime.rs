//! Lambda runtime execution
//!
//! Spawns local processes to execute Lambda handlers.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{Mutex, Notify};

use crate::config::{LambdaConfig, Runtime};
use crate::pool::ProcessPool;

/// Lambda event structure (API Gateway proxy format v1)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LambdaEvent {
    pub http_method: String,
    pub path: String,
    pub resource: String,
    pub path_parameters: Option<HashMap<String, String>>,
    pub query_string_parameters: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub is_base64_encoded: bool,
    pub request_context: RequestContext,
}

/// API Gateway request context (matches real AWS structure)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestContext {
    pub stage: String,
    pub resource_path: String,
    pub http_method: String,
    pub request_id: String,
    pub api_id: String,
    pub path: String,
    pub identity: RequestIdentity,
}

/// Request identity info
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestIdentity {
    pub source_ip: String,
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
#[allow(dead_code)]
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

/// WebSocket event structure (API Gateway WebSocket proxy format)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSocketEvent {
    pub request_context: WebSocketRequestContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub is_base64_encoded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_value_headers: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_string_parameters: Option<HashMap<String, String>>,
}

/// WebSocket request context
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSocketRequestContext {
    pub route_key: String,
    pub event_type: String,
    pub connection_id: String,
    pub stage: String,
    pub api_id: String,
    pub request_id: String,
    pub domain_name: String,
    pub request_time_epoch: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub identity: RequestIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<u64>,
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
#[allow(dead_code)]
pub struct AuthorizerResult {
    pub is_authorized: bool,
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// Debug options for runtime execution
#[derive(Debug, Clone, Default)]
pub struct DebugOptions {
    /// Enable Node.js inspector
    pub nodejs: bool,
    /// Enable Python debugpy
    pub python: bool,
    /// Inspector port for Node.js (default: 9229)
    pub port: u16,
    /// Debug port for Python/debugpy (default: 5678)
    pub python_port: u16,
    /// Break on first line (--inspect-brk vs --inspect)
    pub break_on_start: bool,
}

/// Executor for a Lambda function
pub struct FunctionExecutor {
    config: LambdaConfig,
    source_dir: std::path::PathBuf,
    debug: Option<DebugOptions>,
    pool: Option<Arc<ProcessPool>>,
    /// Resolved layer directories (paths to extracted layer content)
    layer_paths: Vec<std::path::PathBuf>,
}

impl FunctionExecutor {
    pub fn new(config: LambdaConfig, source_dir: std::path::PathBuf) -> Self {
        Self { config, source_dir, debug: None, pool: None, layer_paths: Vec::new() }
    }

    pub fn with_debug(mut self, debug: Option<DebugOptions>) -> Self {
        self.debug = debug;
        self
    }

    pub fn with_pool(mut self, pool: Option<Arc<ProcessPool>>) -> Self {
        self.pool = pool;
        self
    }

    pub fn with_layer_paths(mut self, layer_paths: Vec<std::path::PathBuf>) -> Self {
        self.layer_paths = layer_paths;
        self
    }
    
    /// Build environment variables with layer paths added to NODE_PATH/PYTHONPATH
    fn env_with_layers(&self) -> HashMap<String, String> {
        let mut env = self.config.environment.clone();
        
        if self.layer_paths.is_empty() {
            return env;
        }
        
        // For Node.js layers: content is in nodejs/node_modules
        if self.config.runtime.is_nodejs() {
            let layer_node_paths: Vec<String> = self.layer_paths.iter()
                .flat_map(|p| {
                    // AWS Lambda layers can have code in:
                    // - nodejs/node_modules (most common)
                    // - nodejs/ (for non-module files)
                    let mut paths = Vec::new();
                    let nodejs_modules = p.join("nodejs").join("node_modules");
                    let nodejs = p.join("nodejs");
                    if nodejs_modules.exists() {
                        paths.push(nodejs_modules.to_string_lossy().to_string());
                    }
                    if nodejs.exists() {
                        paths.push(nodejs.to_string_lossy().to_string());
                    }
                    // Also check root node_modules
                    let root_modules = p.join("node_modules");
                    if root_modules.exists() {
                        paths.push(root_modules.to_string_lossy().to_string());
                    }
                    paths
                })
                .collect();
            
            if !layer_node_paths.is_empty() {
                let existing = env.get("NODE_PATH").cloned().unwrap_or_default();
                let combined = if existing.is_empty() {
                    layer_node_paths.join(":")
                } else {
                    format!("{}:{}", layer_node_paths.join(":"), existing)
                };
                env.insert("NODE_PATH".to_string(), combined);
                tracing::debug!("NODE_PATH with layers: {}", env["NODE_PATH"]);
            }
        }
        
        // For Python layers: content is in python/ or python/lib/pythonX.Y/site-packages
        if self.config.runtime.is_python() {
            let layer_python_paths: Vec<String> = self.layer_paths.iter()
                .flat_map(|p| {
                    let mut paths = Vec::new();
                    let python_dir = p.join("python");
                    if python_dir.exists() {
                        paths.push(python_dir.to_string_lossy().to_string());
                    }
                    // Check for lib/pythonX.Y/site-packages
                    let lib_dir = python_dir.join("lib");
                    if lib_dir.exists() {
                        if let Ok(entries) = std::fs::read_dir(&lib_dir) {
                            for entry in entries.flatten() {
                                let sp = entry.path().join("site-packages");
                                if sp.exists() {
                                    paths.push(sp.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                    paths
                })
                .collect();
            
            if !layer_python_paths.is_empty() {
                let existing = env.get("PYTHONPATH").cloned().unwrap_or_default();
                let combined = if existing.is_empty() {
                    layer_python_paths.join(":")
                } else {
                    format!("{}:{}", layer_python_paths.join(":"), existing)
                };
                env.insert("PYTHONPATH".to_string(), combined);
                tracing::debug!("PYTHONPATH with layers: {}", env["PYTHONPATH"]);
            }
        }
        
        env
    }

    /// Whether to use the process pool (not in debug mode).
    fn use_pool(&self) -> bool {
        self.pool.is_some() && self.debug.is_none()
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
            Runtime::Go1 | Runtime::ProvidedAl2 | Runtime::ProvidedAl2023 => {
                // Go authorizer: send auth event directly
                self.invoke_go_with_rie(&serde_json::to_value(&event)?).await?
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
            .envs(&self.env_with_layers())
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
            .envs(&self.env_with_layers())
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
    
    /// Build Go Lambda binary. Returns path to compiled binary.
    async fn build_go(&self) -> Result<std::path::PathBuf> {
        let source_dir = self.source_dir.canonicalize()
            .with_context(|| format!("Source directory not found: {}", self.source_dir.display()))?;
        let binary_path = source_dir.join("bootstrap");
        
        // Check if binary already exists and is newer than source
        let needs_build = if binary_path.exists() {
            let bin_modified = std::fs::metadata(&binary_path)?.modified()?;
            // Check all .go files
            let mut needs = false;
            if let Ok(entries) = std::fs::read_dir(&source_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().map_or(false, |e| e == "go") {
                        if let Ok(meta) = entry.metadata() {
                            if let Ok(src_modified) = meta.modified() {
                                if src_modified > bin_modified {
                                    needs = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            needs
        } else {
            true
        };
        
        if needs_build {
            tracing::info!("Building Go Lambda in {}", source_dir.display());
            let output = Command::new("go")
                .args(["build", "-o", "bootstrap", "."])
                .current_dir(&source_dir)
                .env("GOOS", "linux")
                .env("GOARCH", std::env::consts::ARCH)
                .output()
                .await
                .context("Failed to run `go build`. Is Go installed?")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Go build failed:\n{}", stderr);
            }
            tracing::info!("Go build complete: {}", binary_path.display());
        }
        
        Ok(binary_path)
    }
    
    /// Run a Go Lambda binary with a mini Runtime Interface Emulator.
    /// The binary polls GET /next for the event, then POSTs the response.
    async fn invoke_go_with_rie(&self, event_json: &serde_json::Value) -> Result<serde_json::Value> {
        let binary_path = self.build_go().await?;
        
        let request_id = uuid_simple();
        let event_bytes: Vec<u8> = serde_json::to_vec(event_json)?;
        
        // Shared state for the mini RIE
        let response: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let response_ready = Arc::new(Notify::new());
        
        // Build axum router for the Lambda Runtime API
        use axum::{Router, routing::get, routing::post, extract::State, body::Bytes, http::StatusCode};
        
        #[derive(Clone)]
        struct RieState {
            event: Vec<u8>,
            request_id: String,
            response: Arc<Mutex<Option<serde_json::Value>>>,
            response_ready: Arc<Notify>,
        }
        
        let state = RieState {
            event: event_bytes,
            request_id: request_id,
            response: response.clone(),
            response_ready: response_ready.clone(),
        };
        
        let app = Router::new()
            .route(
                "/2018-06-01/runtime/invocation/next",
                get(|State(s): State<RieState>| async move {
                    (
                        StatusCode::OK,
                        [
                            ("Lambda-Runtime-Aws-Request-Id", s.request_id),
                            ("Lambda-Runtime-Deadline-Ms", "30000".to_string()),
                        ],
                        s.event,
                    )
                })
            )
            .route(
                "/2018-06-01/runtime/invocation/:request_id/response",
                post(|State(s): State<RieState>, body: Bytes| async move {
                    let val: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
                    *s.response.lock().await = Some(val);
                    s.response_ready.notify_one();
                    StatusCode::ACCEPTED
                })
            )
            .route(
                "/2018-06-01/runtime/invocation/:request_id/error",
                post(|State(s): State<RieState>, body: Bytes| async move {
                    let err_str = String::from_utf8_lossy(&body).to_string();
                    *s.response.lock().await = Some(serde_json::json!({"errorMessage": err_str}));
                    s.response_ready.notify_one();
                    StatusCode::ACCEPTED
                })
            )
            .with_state(state);
        
        // Bind to random port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        
        // Run the Go binary
        let mut child = Command::new(&binary_path)
            .current_dir(&self.source_dir)
            .env("AWS_LAMBDA_RUNTIME_API", format!("127.0.0.1:{}", port))
            .env("_HANDLER", &self.config.handler)
            .envs(&self.env_with_layers())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn Go binary: {}", binary_path.display()))?;
        
        // Wait for response with timeout
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.timeout as u64 + 5),
            response_ready.notified()
        ).await;
        
        // Kill the child process and server
        child.kill().await.ok();
        server_handle.abort();
        
        if timeout.is_err() {
            anyhow::bail!("Go Lambda timed out after {}s", self.config.timeout);
        }
        
        let result = response.lock().await.take()
            .ok_or_else(|| anyhow::anyhow!("No response from Go Lambda"))?;
        
        // Check for error
        if let Some(err) = result.get("errorMessage").and_then(|v| v.as_str()) {
            anyhow::bail!("Go Lambda error: {}", err);
        }
        
        Ok(result)
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
        
        // Use pool for Node.js/Python when available and not debugging
        if self.use_pool() {
            match &self.config.runtime {
                Runtime::Nodejs18 | Runtime::Nodejs20 | Runtime::Python310 | Runtime::Python311 | Runtime::Python312 => {
                    let pool = self.pool.as_ref().unwrap();
                    let env = self.env_with_layers();
                    let result = pool.invoke(
                        &self.config.function_name,
                        &self.config.runtime,
                        &self.config.handler,
                        &self.source_dir,
                        &env,
                        &payload["event"],
                        &payload["context"],
                    ).await?;
                    let response: LambdaResponse = serde_json::from_value(result)?;
                    return Ok(response);
                }
                _ => {}
            }
        }

        match &self.config.runtime {
            Runtime::Nodejs18 | Runtime::Nodejs20 => {
                self.invoke_nodejs(&payload).await
            }
            Runtime::Python310 | Runtime::Python311 | Runtime::Python312 => {
                self.invoke_python(&payload).await
            }
            Runtime::Go1 | Runtime::ProvidedAl2 | Runtime::ProvidedAl2023 => {
                self.invoke_go(&payload).await
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
        
        // Node.js bootstrap script
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
        
        // Check if debug mode is enabled for Node.js
        let debug_enabled = self.debug.as_ref().map_or(false, |d| d.nodejs);
        
        let mut cmd = Command::new("node");
        
        if debug_enabled {
            let debug_opts = self.debug.as_ref().unwrap();
            let flag = if debug_opts.break_on_start {
                format!("--inspect-brk=0.0.0.0:{}", debug_opts.port)
            } else {
                format!("--inspect=0.0.0.0:{}", debug_opts.port)
            };
            cmd.arg(&flag);
            tracing::info!("🔍 Node.js debugger listening on ws://0.0.0.0:{}", debug_opts.port);
            tracing::info!("   Open chrome://inspect or attach VS Code to debug");
        }
        
        // When debugging, write bootstrap to a temp file for better source visibility
        // in debugger. Otherwise, use inline -e for speed.
        let _temp_file; // hold reference to keep temp file alive
        if debug_enabled {
            let tmp = std::env::temp_dir().join(format!("lambdaform-bootstrap-{}.js", 
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_nanos()));
            std::fs::write(&tmp, &bootstrap)
                .context("Failed to write debug bootstrap file")?;
            cmd.arg(&tmp);
            _temp_file = Some(tmp);
        } else {
            cmd.arg("-e").arg(&bootstrap);
            _temp_file = None;
        }
        
        let mut child = cmd
            .current_dir(&self.source_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&self.env_with_layers())
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
        
        // Check if debug mode is enabled for Python
        let debug_enabled = self.debug.as_ref().map_or(false, |d| d.python);
        
        let debug_preamble = if debug_enabled {
            let debug_opts = self.debug.as_ref().unwrap();
            let port = debug_opts.python_port;
            let wait = if debug_opts.break_on_start { "True" } else { "False" };
            tracing::info!("🐍 Python debugger (debugpy) listening on 0.0.0.0:{}", port);
            tracing::info!("   Attach VS Code or any DAP client to debug");
            format!(
                r#"
import debugpy
debugpy.listen(("0.0.0.0", {}))
if {}:
    print("⏳ Waiting for debugger to attach...", file=__import__('sys').stderr)
    debugpy.wait_for_client()
"#,
                port, wait
            )
        } else {
            String::new()
        };
        
        let bootstrap = format!(
            r#"
import sys
import json
import importlib.util
{}
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
            debug_preamble,
            handler_path.display(),
            func
        );
        
        // When debugging, write bootstrap to a temp file for better source visibility
        let _temp_file;
        let mut cmd = Command::new("python3");
        
        if debug_enabled {
            let tmp = std::env::temp_dir().join(format!("lambdaform-bootstrap-{}.py",
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_nanos()));
            std::fs::write(&tmp, &bootstrap)
                .context("Failed to write debug bootstrap file")?;
            cmd.arg(&tmp);
            _temp_file = Some(tmp);
        } else {
            cmd.arg("-c").arg(&bootstrap);
            _temp_file = None;
        }
        
        let mut child = cmd
            .current_dir(&self.source_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&self.env_with_layers())
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
    
    /// Invoke Go function via Runtime Interface Emulator
    async fn invoke_go(&self, payload: &serde_json::Value) -> Result<LambdaResponse> {
        // For Go, the event is the API Gateway event directly (not wrapped in {event, context})
        let event = &payload["event"];
        let result = self.invoke_go_with_rie(event).await?;
        let response: LambdaResponse = serde_json::from_value(result)?;
        Ok(response)
    }
}

/// Find handler file by searching common locations. Returns canonical (absolute) path.
pub fn find_handler_file(source_dir: &std::path::Path, file: &str, ext: &str) -> Result<std::path::PathBuf> {
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
pub fn parse_handler(handler: &str) -> Result<(&str, &str)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_parse_handler_basic() {
        let (file, func) = parse_handler("index.handler").unwrap();
        assert_eq!(file, "index");
        assert_eq!(func, "handler");
    }

    #[test]
    fn test_parse_handler_nested() {
        let (file, func) = parse_handler("src/app.main").unwrap();
        assert_eq!(file, "src/app");
        assert_eq!(func, "main");
    }

    #[test]
    fn test_parse_handler_invalid() {
        assert!(parse_handler("nohandler").is_err());
    }

    #[test]
    fn test_find_handler_file_root() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("index.js"), "// handler").unwrap();
        let result = find_handler_file(dir.path(), "index", "js").unwrap();
        assert!(result.exists());
        assert!(result.to_string_lossy().contains("index.js"));
    }

    #[test]
    fn test_find_handler_file_src_subdir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src").join("app.py"), "# handler").unwrap();
        let result = find_handler_file(dir.path(), "app", "py").unwrap();
        assert!(result.exists());
    }

    #[test]
    fn test_find_handler_file_not_found() {
        let dir = TempDir::new().unwrap();
        assert!(find_handler_file(dir.path(), "missing", "js").is_err());
    }

    #[test]
    fn test_uuid_simple_format() {
        let id = uuid_simple();
        assert!(id.contains('-'));
        // Should be hex characters and a dash
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn test_lambda_event_serialization() {
        let event = LambdaEvent {
            http_method: "GET".to_string(),
            path: "/test".to_string(),
            resource: "/test".to_string(),
            path_parameters: None,
            query_string_parameters: Some(HashMap::from([("key".to_string(), "val".to_string())])),
            headers: None,
            body: None,
            is_base64_encoded: false,
            request_context: RequestContext {
                stage: "local".to_string(),
                resource_path: "/test".to_string(),
                http_method: "GET".to_string(),
                request_id: "test-123".to_string(),
                api_id: "lambdaform".to_string(),
                path: "/test".to_string(),
                identity: RequestIdentity { source_ip: "127.0.0.1".to_string() },
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["httpMethod"], "GET");
        assert_eq!(json["path"], "/test");
        assert_eq!(json["queryStringParameters"]["key"], "val");
        assert_eq!(json["requestContext"]["stage"], "local");
    }

    #[test]
    fn test_lambda_response_deserialization() {
        let json = r#"{"statusCode": 200, "body": "hello", "headers": {"x-custom": "value"}}"#;
        let resp: LambdaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body, Some("hello".to_string()));
        assert_eq!(resp.headers.unwrap()["x-custom"], "value");
        assert!(!resp.is_base64_encoded);
    }

    #[test]
    fn test_lambda_response_minimal() {
        let json = r#"{"statusCode": 204}"#;
        let resp: LambdaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status_code, 204);
        assert!(resp.body.is_none());
        assert!(resp.headers.is_none());
    }

    #[test]
    fn test_authorizer_event_serialization() {
        let event = AuthorizerEvent {
            auth_type: "TOKEN".to_string(),
            authorization_token: Some("Bearer xyz".to_string()),
            method_arn: "arn:aws:execute-api:local:000:api/GET/test".to_string(),
            http_method: "GET".to_string(),
            path: "/test".to_string(),
            headers: None,
            query_string_parameters: None,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "TOKEN");
        assert_eq!(json["authorizationToken"], "Bearer xyz");
    }
}
