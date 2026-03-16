//! Warm process pool for Node.js and Python Lambda workers.
//!
//! Keeps long-lived worker processes that accept JSON-line invocations
//! over stdin/stdout, eliminating per-request process spawn overhead.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// A single warm worker process.
struct Worker {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    _stderr_drain: tokio::task::JoinHandle<()>,
}

impl Worker {
    /// Send an invocation and read the response.
    async fn invoke(
        &mut self,
        id: &str,
        event: &serde_json::Value,
        context: &serde_json::Value,
    ) -> Result<WorkerResponse> {
        let request = serde_json::json!({
            "id": id,
            "event": event,
            "context": context,
        });
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .context("Failed to write to worker stdin")?;
        self.stdin.flush().await?;

        // Read response line
        let mut response_line = String::new();
        let n = self
            .stdout
            .read_line(&mut response_line)
            .await
            .context("Failed to read from worker stdout")?;
        if n == 0 {
            anyhow::bail!("Worker process closed stdout (crashed?)");
        }

        let resp: WorkerResponse = serde_json::from_str(response_line.trim())
            .with_context(|| format!("Invalid worker response: {}", response_line.trim()))?;

        if resp.id != id {
            anyhow::bail!(
                "Worker response id mismatch: expected {}, got {}",
                id,
                resp.id
            );
        }

        Ok(resp)
    }

    /// Check if the process is still alive.
    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

#[derive(Debug, serde::Deserialize)]
struct WorkerResponse {
    id: String,
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

/// Key for caching workers: (function_name, handler).
type WorkerKey = (String, String);

/// Pool of warm worker processes.
pub struct ProcessPool {
    workers: Mutex<HashMap<WorkerKey, Arc<Mutex<Worker>>>>,
}

impl Default for ProcessPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessPool {
    pub fn new() -> Self {
        Self {
            workers: Mutex::new(HashMap::new()),
        }
    }

    /// Invoke a function using a pooled worker. Spawns one if needed.
    #[allow(clippy::too_many_arguments)]
    pub async fn invoke(
        &self,
        function_name: &str,
        runtime: &crate::config::Runtime,
        handler: &str,
        source_dir: &Path,
        env: &HashMap<String, String>,
        event: &serde_json::Value,
        context: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let key = (function_name.to_string(), handler.to_string());
        let id = format!(
            "req-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        // Lock the pool only to get/insert the worker handle, then release.
        let worker_handle = {
            let mut workers = self.workers.lock().await;

            // Check if existing worker is alive
            let needs_spawn = if let Some(w) = workers.get(&key) {
                let mut w = w.lock().await;
                if w.is_alive() {
                    false
                } else {
                    tracing::debug!("Worker for {} died, respawning", function_name);
                    drop(w);
                    workers.remove(&key);
                    true
                }
            } else {
                true
            };

            if needs_spawn {
                let worker = spawn_worker(runtime, handler, source_dir, env)
                    .await
                    .with_context(|| format!("Failed to spawn worker for {}", function_name))?;
                workers.insert(key.clone(), Arc::new(Mutex::new(worker)));
            }

            Arc::clone(workers.get(&key).expect("Worker was just inserted"))
        };
        // Pool mutex is now released — concurrent requests to OTHER functions proceed freely.
        // The per-worker mutex serializes requests to the SAME function (necessary since
        // stdin/stdout is a single stream), but doesn't block unrelated functions.

        let mut worker = worker_handle.lock().await;
        let resp = worker.invoke(&id, event, context).await;

        // If invoke failed, remove the worker so it gets respawned next time
        match resp {
            Ok(r) => {
                if r.success {
                    r.result
                        .ok_or_else(|| anyhow::anyhow!("Worker returned success but no result"))
                } else {
                    anyhow::bail!("Lambda error: {}", r.error.unwrap_or_default())
                }
            }
            Err(e) => {
                drop(worker);
                let mut workers = self.workers.lock().await;
                workers.remove(&key);
                Err(e)
            }
        }
    }

    /// Kill all workers (for hot reload).
    pub async fn invalidate_all(&self) {
        let mut workers = self.workers.lock().await;
        for (key, worker) in workers.drain() {
            tracing::debug!("Killing worker: {}:{}", key.0, key.1);
            let mut w = worker.lock().await;
            let _ = w.child.kill().await;
        }
    }
}

impl Drop for ProcessPool {
    fn drop(&mut self) {
        // Best-effort kill — we can't async here, so just start kill signals
        if let Ok(mut workers) = self.workers.try_lock() {
            for (_, worker) in workers.drain() {
                if let Ok(mut w) = worker.try_lock() {
                    let _ = w.child.start_kill();
                }
            }
        }
    }
}

/// Spawn a new worker process for the given runtime.
async fn spawn_worker(
    runtime: &crate::config::Runtime,
    handler: &str,
    source_dir: &Path,
    env: &HashMap<String, String>,
) -> Result<Worker> {
    match runtime {
        crate::config::Runtime::Nodejs18
        | crate::config::Runtime::Nodejs20
        | crate::config::Runtime::Nodejs22 => spawn_nodejs_worker(handler, source_dir, env).await,
        crate::config::Runtime::Python310
        | crate::config::Runtime::Python311
        | crate::config::Runtime::Python312
        | crate::config::Runtime::Python313 => spawn_python_worker(handler, source_dir, env).await,
        _ => anyhow::bail!("Process pool not supported for runtime {:?}", runtime),
    }
}

/// Spawn a background task that reads stderr line-by-line and forwards to tracing.
/// This prevents the OS pipe buffer (typically 64KB) from filling up and blocking the worker.
fn drain_stderr(
    stderr: tokio::process::ChildStderr,
    function_name: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim_end();
                    if !trimmed.is_empty() {
                        tracing::info!(target: "lambda", "[{}] {}", function_name, trimmed);
                    }
                }
                Err(_) => break,
            }
        }
    })
}

/// Wait for the worker startup handshake (ready signal) with a timeout.
async fn wait_for_handshake(
    stdout: &mut BufReader<tokio::process::ChildStdout>,
    runtime_name: &str,
) -> Result<()> {
    let mut line = String::new();
    let read_result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        stdout
            .read_line(&mut line)
            .await
            .context("Failed to read handshake from worker")
    })
    .await;

    match read_result {
        Err(_) => anyhow::bail!(
            "{} worker failed to start within 30s (possible import hang — check for network calls or blocking operations at module level)",
            runtime_name
        ),
        Ok(Err(e)) => return Err(e),
        Ok(Ok(0)) => anyhow::bail!(
            "{} worker exited during startup (check stderr for import/syntax errors)",
            runtime_name
        ),
        Ok(Ok(_)) => {}
    }

    #[derive(serde::Deserialize)]
    struct Handshake {
        ready: bool,
        error: Option<String>,
    }

    let hs: Handshake = serde_json::from_str(line.trim()).with_context(|| {
        format!(
            "Invalid handshake from {} worker: {}",
            runtime_name,
            line.trim()
        )
    })?;

    if !hs.ready {
        anyhow::bail!(
            "{} worker failed to initialize: {}",
            runtime_name,
            hs.error.unwrap_or_else(|| "unknown error".to_string())
        );
    }

    Ok(())
}

// Re-use shared handler parsing from runtime module
use crate::runtime::{find_handler_file, parse_handler};

async fn spawn_nodejs_worker(
    handler: &str,
    source_dir: &Path,
    env: &HashMap<String, String>,
) -> Result<Worker> {
    let (file, func) = parse_handler(handler)?;
    let handler_path = find_handler_file(source_dir, file, "js")?;

    let bootstrap = format!(
        r#"
const readline = require('readline');
const fs = require('fs');

// Capture real stdout fd for protocol, redirect console.log to stderr
const _realStdout = fs.createWriteStream(null, {{ fd: 1 }});
const _origWrite = process.stdout.write.bind(process.stdout);
// Override console to write to stderr
const origLog = console.log;
const origWarn = console.warn;
const origInfo = console.info;
const origDir = console.dir;
console.log = (...args) => process.stderr.write(require('util').format(...args) + '\n');
console.warn = (...args) => process.stderr.write(require('util').format(...args) + '\n');
console.info = (...args) => process.stderr.write(require('util').format(...args) + '\n');
console.dir = (...args) => process.stderr.write(require('util').format(...args) + '\n');
// Also intercept process.stdout.write from user code
process.stdout.write = (chunk, enc, cb) => process.stderr.write(chunk, enc, cb);

function sendResponse(obj) {{
    _origWrite(JSON.stringify(obj) + '\n');
}}

let handlerFn;
try {{
    const handler = require('{handler_path}');
    handlerFn = handler['{func}'];
    if (typeof handlerFn !== 'function') {{
        throw new Error('Handler {func} is not a function (got ' + typeof handlerFn + ')');
    }}
    sendResponse({{ ready: true }});
}} catch (e) {{
    sendResponse({{ ready: false, error: e.message }});
    process.exit(1);
}}

const rl = readline.createInterface({{ input: process.stdin, terminal: false }});

rl.on('line', async (line) => {{
    let req;
    try {{
        req = JSON.parse(line);
    }} catch (e) {{
        return;
    }}
    try {{
        const result = await handlerFn(req.event, req.context);
        sendResponse({{ id: req.id, success: true, result }});
    }} catch (error) {{
        sendResponse({{ id: req.id, success: false, error: error.message }});
    }}
}});
"#,
        handler_path = handler_path.display(),
        func = func
    );

    let mut child = Command::new("node")
        .arg("-e")
        .arg(&bootstrap)
        .current_dir(source_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .envs(env)
        .spawn()
        .context("Failed to spawn Node.js worker")?;

    let stdin = child
        .stdin
        .take()
        .context("Failed to capture worker stdin")?;
    let mut stdout = BufReader::new(
        child
            .stdout
            .take()
            .context("Failed to capture worker stdout")?,
    );
    let stderr = child
        .stderr
        .take()
        .context("Failed to capture worker stderr")?;
    let _stderr_drain = drain_stderr(stderr, "node-worker".to_string());

    wait_for_handshake(&mut stdout, "Node.js").await?;

    Ok(Worker {
        child,
        stdin,
        stdout,
        _stderr_drain,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_new() {
        let pool = ProcessPool::new();
        let workers = pool.workers.lock().await;
        assert!(workers.is_empty());
    }

    #[tokio::test]
    async fn test_invalidate_all_empty() {
        let pool = ProcessPool::new();
        pool.invalidate_all().await;
        let workers = pool.workers.lock().await;
        assert!(workers.is_empty());
    }

    #[tokio::test]
    async fn test_worker_key_uniqueness() {
        // Verify different function/handler combos produce different keys
        let key1: WorkerKey = ("func_a".to_string(), "index.handler".to_string());
        let key2: WorkerKey = ("func_b".to_string(), "index.handler".to_string());
        let key3: WorkerKey = ("func_a".to_string(), "other.handler".to_string());
        assert_ne!(key1, key2);
        assert_ne!(key1, key3);
        assert_ne!(key2, key3);
    }

    #[test]
    fn test_worker_response_deserialize() {
        let json = r#"{"id":"req-1","success":true,"result":{"statusCode":200}}"#;
        let resp: WorkerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "req-1");
        assert!(resp.success);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_worker_response_error_deserialize() {
        let json = r#"{"id":"req-2","success":false,"error":"timeout"}"#;
        let resp: WorkerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "req-2");
        assert!(!resp.success);
        assert!(resp.result.is_none());
        assert_eq!(resp.error.as_deref(), Some("timeout"));
    }

    #[test]
    fn test_worker_response_with_null_fields() {
        let json = r#"{"id":"req-3","success":true,"result":null,"error":null}"#;
        let resp: WorkerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "req-3");
        assert!(resp.success);
        assert!(resp.result.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_worker_response_complex_result() {
        let json = r#"{"id":"req-4","success":true,"result":{"statusCode":200,"headers":{"Content-Type":"application/json"},"body":"{\"items\":[1,2,3]}"}}"#;
        let resp: WorkerResponse = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        let result = resp.result.unwrap();
        assert_eq!(result["statusCode"], 200);
        assert_eq!(result["headers"]["Content-Type"], "application/json");
    }

    #[test]
    fn test_worker_response_missing_optional_fields() {
        // Both result and error omitted entirely
        let json = r#"{"id":"req-5","success":true}"#;
        let resp: WorkerResponse = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        assert!(resp.result.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_pool_default_trait() {
        let pool = ProcessPool::default();
        // Should behave identically to new()
        let workers = pool.workers.try_lock().unwrap();
        assert!(workers.is_empty());
    }

    #[test]
    fn test_worker_key_same_function_same_handler() {
        let key1: WorkerKey = ("func_a".to_string(), "index.handler".to_string());
        let key2: WorkerKey = ("func_a".to_string(), "index.handler".to_string());
        assert_eq!(key1, key2);
    }

    #[tokio::test]
    async fn test_invalidate_all_is_idempotent() {
        let pool = ProcessPool::new();
        pool.invalidate_all().await;
        pool.invalidate_all().await;
        let workers = pool.workers.lock().await;
        assert!(workers.is_empty());
    }

    #[test]
    fn test_worker_response_invalid_json_fails() {
        let json = r#"{"id":"req-6","success":"not_a_bool"}"#;
        let result: Result<WorkerResponse, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    /// Helper to create a temp dir with a Node.js handler file.
    fn create_node_handler(dir: &std::path::Path, filename: &str, code: &str) {
        std::fs::write(dir.join(filename), code).unwrap();
    }

    #[tokio::test]
    async fn test_pool_invoke_nodejs_success() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"exports.handler = async (event, context) => {
                return { statusCode: 200, body: JSON.stringify({ msg: "hello", input: event }) };
            };"#,
        );

        let pool = ProcessPool::new();
        let env = HashMap::new();
        let event = serde_json::json!({"key": "value"});
        let context = serde_json::json!({"functionName": "test"});

        let result = pool
            .invoke(
                "test_func",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &event,
                &context,
            )
            .await
            .unwrap();

        assert_eq!(result["statusCode"], 200);
        let body: serde_json::Value =
            serde_json::from_str(result["body"].as_str().unwrap()).unwrap();
        assert_eq!(body["input"]["key"], "value");
    }

    #[tokio::test]
    async fn test_pool_invoke_python_success() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("handler.py"),
            r#"
def handle(event, context):
    return {"statusCode": 200, "body": str(event.get("key", ""))}
"#,
        )
        .unwrap();

        let pool = ProcessPool::new();
        let env = HashMap::new();
        let event = serde_json::json!({"key": "pyval"});
        let context = serde_json::json!({});

        let result = pool
            .invoke(
                "py_func",
                &crate::config::Runtime::Python312,
                "handler.handle",
                tmp.path(),
                &env,
                &event,
                &context,
            )
            .await
            .unwrap();

        assert_eq!(result["statusCode"], 200);
        assert_eq!(result["body"], "pyval");
    }

    #[tokio::test]
    async fn test_pool_reuses_worker() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"
            let count = 0;
            exports.handler = async (event) => {
                count++;
                return { statusCode: 200, body: String(count) };
            };"#,
        );

        let pool = ProcessPool::new();
        let env = HashMap::new();
        let event = serde_json::json!({});
        let context = serde_json::json!({});

        // First call
        let r1 = pool
            .invoke(
                "counter",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &event,
                &context,
            )
            .await
            .unwrap();
        assert_eq!(r1["body"], "1");

        // Second call — same worker, counter increments
        let r2 = pool
            .invoke(
                "counter",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &event,
                &context,
            )
            .await
            .unwrap();
        assert_eq!(r2["body"], "2");
    }

    #[tokio::test]
    async fn test_pool_invalidate_resets_workers() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"
            let count = 0;
            exports.handler = async () => { count++; return { body: String(count) }; };"#,
        );

        let pool = ProcessPool::new();
        let env = HashMap::new();
        let event = serde_json::json!({});
        let ctx = serde_json::json!({});

        let r1 = pool
            .invoke(
                "f",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &event,
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r1["body"], "1");

        // Invalidate kills workers
        pool.invalidate_all().await;

        // Next call spawns fresh worker — counter resets
        let r2 = pool
            .invoke(
                "f",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &event,
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r2["body"], "1");
    }

    #[tokio::test]
    async fn test_pool_handler_error_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"exports.handler = async () => { throw new Error("boom"); };"#,
        );

        let pool = ProcessPool::new();
        let env = HashMap::new();

        let result = pool
            .invoke(
                "err_func",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("boom"));
    }

    #[tokio::test]
    async fn test_pool_env_vars_passed_to_worker() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"exports.handler = async () => {
                return { statusCode: 200, body: process.env.MY_VAR || "missing" };
            };"#,
        );

        let pool = ProcessPool::new();
        let mut env = HashMap::new();
        env.insert("MY_VAR".to_string(), "test_value".to_string());

        let result = pool
            .invoke(
                "env_func",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await
            .unwrap();

        assert_eq!(result["body"], "test_value");
    }

    #[tokio::test]
    async fn test_pool_unsupported_runtime_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = ProcessPool::new();

        let result = pool
            .invoke(
                "go_func",
                &crate::config::Runtime::Go1,
                "main",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        let chain: String = err
            .chain()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(
            chain.contains("not supported") || chain.contains("Pool"),
            "Unexpected error chain: {chain}"
        );
    }

    #[tokio::test]
    async fn test_pool_concurrent_different_functions() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "a.js",
            r#"exports.handler = async () => ({ body: "a" });"#,
        );
        create_node_handler(
            tmp.path(),
            "b.js",
            r#"exports.handler = async () => ({ body: "b" });"#,
        );

        let pool = Arc::new(ProcessPool::new());
        let env = HashMap::new();
        let event = serde_json::json!({});
        let ctx = serde_json::json!({});

        let pool_a = Arc::clone(&pool);
        let tmp_a = tmp.path().to_path_buf();
        let env_a = env.clone();
        let event_a = event.clone();
        let ctx_a = ctx.clone();

        let pool_b = Arc::clone(&pool);
        let tmp_b = tmp.path().to_path_buf();
        let env_b = env.clone();
        let event_b = event.clone();
        let ctx_b = ctx.clone();

        let (ra, rb) = tokio::join!(
            async move {
                pool_a
                    .invoke(
                        "fa",
                        &crate::config::Runtime::Nodejs20,
                        "a.handler",
                        &tmp_a,
                        &env_a,
                        &event_a,
                        &ctx_a,
                    )
                    .await
                    .unwrap()
            },
            async move {
                pool_b
                    .invoke(
                        "fb",
                        &crate::config::Runtime::Nodejs20,
                        "b.handler",
                        &tmp_b,
                        &env_b,
                        &event_b,
                        &ctx_b,
                    )
                    .await
                    .unwrap()
            }
        );

        assert_eq!(ra["body"], "a");
        assert_eq!(rb["body"], "b");
    }
}

async fn spawn_python_worker(
    handler: &str,
    source_dir: &Path,
    env: &HashMap<String, String>,
) -> Result<Worker> {
    let (file, func) = parse_handler(handler)?;
    let handler_path = find_handler_file(source_dir, file, "py")?;

    let bootstrap = format!(
        r#"
import sys
import os
import json
import importlib.util

# Save real stdout for protocol, redirect user stdout/print to stderr
_real_stdout = os.fdopen(os.dup(1), 'w')
sys.stdout = sys.stderr  # print() goes to stderr now

try:
    spec = importlib.util.spec_from_file_location("handler", "{handler_path}")
    if spec is None:
        raise ImportError("Could not find module: {handler_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    handler_fn = getattr(module, "{func}")
    _real_stdout.write(json.dumps({{"ready": True}}) + "\n")
    _real_stdout.flush()
except Exception as e:
    _real_stdout.write(json.dumps({{"ready": False, "error": str(e)}}) + "\n")
    _real_stdout.flush()
    sys.exit(1)

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    try:
        result = handler_fn(req["event"], req["context"])
        _real_stdout.write(json.dumps({{"id": req["id"], "success": True, "result": result}}) + "\n")
        _real_stdout.flush()
    except Exception as e:
        _real_stdout.write(json.dumps({{"id": req["id"], "success": False, "error": str(e)}}) + "\n")
        _real_stdout.flush()
"#,
        handler_path = handler_path.display(),
        func = func
    );

    let mut child = Command::new("python3")
        .arg("-u") // unbuffered
        .arg("-c")
        .arg(&bootstrap)
        .current_dir(source_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .envs(env)
        .spawn()
        .context("Failed to spawn Python worker")?;

    let stdin = child
        .stdin
        .take()
        .context("Failed to capture worker stdin")?;
    let mut stdout = BufReader::new(
        child
            .stdout
            .take()
            .context("Failed to capture worker stdout")?,
    );
    let stderr = child
        .stderr
        .take()
        .context("Failed to capture worker stderr")?;
    let _stderr_drain = drain_stderr(stderr, "python-worker".to_string());

    wait_for_handshake(&mut stdout, "Python").await?;

    Ok(Worker {
        child,
        stdin,
        stdout,
        _stderr_drain,
    })
}

// Additional pool tests - error handling and edge cases
#[cfg(test)]
mod tests_extended {
    use super::*;

    fn create_node_handler(dir: &std::path::Path, filename: &str, code: &str) {
        std::fs::write(dir.join(filename), code).unwrap();
    }

    #[tokio::test]
    async fn test_pool_nodejs_import_error_fails() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"throw new Error("module load failure");"#,
        );

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "bad_import",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_python_import_error_fails() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("bad.py"),
            "raise RuntimeError('import boom')\n",
        )
        .unwrap();

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "bad_py",
                &crate::config::Runtime::Python312,
                "bad.handle",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_missing_handler_function_fails() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"exports.other = async () => ({ body: "nope" });"#,
        );

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "missing_fn",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not a function")
                || err_msg.contains("Failed to spawn")
                || err_msg.contains("failed to initialize"),
            "unexpected error: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_pool_python_handler_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("err.py"),
            "def handle(event, context):\n    raise ValueError('python boom')\n",
        )
        .unwrap();

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "py_err",
                &crate::config::Runtime::Python312,
                "err.handle",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("python boom"));
    }

    #[tokio::test]
    async fn test_pool_respawns_after_worker_crash() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"exports.handler = async () => ({ body: "alive" });"#,
        );

        let pool = ProcessPool::new();
        let env = HashMap::new();
        let event = serde_json::json!({});
        let ctx = serde_json::json!({});

        let r1 = pool
            .invoke(
                "crash_test",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &event,
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r1["body"], "alive");

        // Kill the worker process directly
        {
            let workers = pool.workers.lock().await;
            let key = ("crash_test".to_string(), "index.handler".to_string());
            if let Some(w) = workers.get(&key) {
                let mut w = w.lock().await;
                let _ = w.child.kill().await;
                let _ = w.child.wait().await;
            }
        }

        // Should respawn and succeed
        let r2 = pool
            .invoke(
                "crash_test",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &env,
                &event,
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r2["body"], "alive");
    }

    #[tokio::test]
    async fn test_pool_python_env_vars() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("env_check.py"),
            "import os\ndef handle(event, context):\n    return {\"body\": os.environ.get(\"TABLE_NAME\", \"unset\")}\n",
        )
        .unwrap();

        let pool = ProcessPool::new();
        let mut env = HashMap::new();
        env.insert("TABLE_NAME".to_string(), "my-table".to_string());

        let result = pool
            .invoke(
                "py_env",
                &crate::config::Runtime::Python312,
                "env_check.handle",
                tmp.path(),
                &env,
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await
            .unwrap();

        assert_eq!(result["body"], "my-table");
    }

    #[tokio::test]
    async fn test_pool_missing_handler_file_fails() {
        let tmp = tempfile::tempdir().unwrap();

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "ghost",
                &crate::config::Runtime::Nodejs20,
                "nonexistent.handler",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_nodejs_async_handler() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "index.js",
            r#"exports.handler = async (event) => {
                await new Promise(resolve => setTimeout(resolve, 10));
                return { statusCode: 200, body: "async-done" };
            };"#,
        );

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "async_fn",
                &crate::config::Runtime::Nodejs20,
                "index.handler",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await
            .unwrap();

        assert_eq!(result["body"], "async-done");
    }
    #[tokio::test]
    async fn test_pool_context_object_passed_through() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "ctx.js",
            r#"exports.handler = async (event, context) => ({
                body: JSON.stringify({
                    fn_name: context.functionName,
                    timeout: context.timeout,
                    region: context.region
                })
            });"#,
        );

        let pool = ProcessPool::new();
        let context = serde_json::json!({
            "functionName": "my_lambda",
            "timeout": 30,
            "region": "us-west-2"
        });

        let result = pool
            .invoke(
                "ctx_fn",
                &crate::config::Runtime::Nodejs20,
                "ctx.handler",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &context,
            )
            .await
            .unwrap();

        let body: serde_json::Value =
            serde_json::from_str(result["body"].as_str().unwrap()).unwrap();
        assert_eq!(body["fn_name"], "my_lambda");
        assert_eq!(body["timeout"], 30);
        assert_eq!(body["region"], "us-west-2");
    }

    #[tokio::test]
    async fn test_pool_python_async_like_handler() {
        // Python handlers are synchronous but should handle complex return types
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("complex.py"),
            r#"
def handle(event, context):
    items = [{"id": i, "name": f"item-{i}"} for i in range(5)]
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": str(len(items)),
        "isBase64Encoded": False
    }
"#,
        )
        .unwrap();

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "py_complex",
                &crate::config::Runtime::Python312,
                "complex.handle",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await
            .unwrap();

        assert_eq!(result["statusCode"], 200);
        assert_eq!(result["body"], "5");
        assert_eq!(result["headers"]["Content-Type"], "application/json");
    }

    #[tokio::test]
    async fn test_pool_recover_after_error_then_success() {
        // After a handler throws, the same function should still work on next call
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "conditional.js",
            r#"exports.handler = async (event) => {
                if (event.shouldFail) throw new Error("conditional fail");
                return { body: "ok" };
            };"#,
        );

        let pool = ProcessPool::new();
        let env = HashMap::new();
        let ctx = serde_json::json!({});

        // First call: error
        let err_result = pool
            .invoke(
                "recover_fn",
                &crate::config::Runtime::Nodejs20,
                "conditional.handler",
                tmp.path(),
                &env,
                &serde_json::json!({"shouldFail": true}),
                &ctx,
            )
            .await;
        assert!(err_result.is_err());

        // Second call: should succeed (worker respawns or reuses)
        let ok_result = pool
            .invoke(
                "recover_fn",
                &crate::config::Runtime::Nodejs20,
                "conditional.handler",
                tmp.path(),
                &env,
                &serde_json::json!({"shouldFail": false}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(ok_result["body"], "ok");
    }

    #[tokio::test]
    async fn test_pool_multiple_env_vars() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "multi_env.js",
            r#"exports.handler = async () => ({
                body: JSON.stringify({
                    table: process.env.TABLE_NAME || "",
                    region: process.env.AWS_REGION || "",
                    stage: process.env.STAGE || ""
                })
            });"#,
        );

        let pool = ProcessPool::new();
        let mut env = HashMap::new();
        env.insert("TABLE_NAME".to_string(), "users".to_string());
        env.insert("AWS_REGION".to_string(), "us-east-1".to_string());
        env.insert("STAGE".to_string(), "prod".to_string());

        let result = pool
            .invoke(
                "multi_env_fn",
                &crate::config::Runtime::Nodejs20,
                "multi_env.handler",
                tmp.path(),
                &env,
                &serde_json::json!({}),
                &serde_json::json!({}),
            )
            .await
            .unwrap();

        let body: serde_json::Value =
            serde_json::from_str(result["body"].as_str().unwrap()).unwrap();
        assert_eq!(body["table"], "users");
        assert_eq!(body["region"], "us-east-1");
        assert_eq!(body["stage"], "prod");
    }

    #[tokio::test]
    async fn test_pool_python_context_access() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("ctx_py.py"),
            r#"
def handle(event, context):
    return {"body": context.get("functionName", "unknown")}
"#,
        )
        .unwrap();

        let pool = ProcessPool::new();
        let result = pool
            .invoke(
                "py_ctx",
                &crate::config::Runtime::Python312,
                "ctx_py.handle",
                tmp.path(),
                &HashMap::new(),
                &serde_json::json!({}),
                &serde_json::json!({"functionName": "my_py_lambda"}),
            )
            .await
            .unwrap();

        assert_eq!(result["body"], "my_py_lambda");
    }

    #[tokio::test]
    async fn test_pool_sequential_rapid_invocations() {
        let tmp = tempfile::tempdir().unwrap();
        create_node_handler(
            tmp.path(),
            "rapid.js",
            r#"
            let seq = 0;
            exports.handler = async (event) => {
                seq++;
                return { body: String(seq), input: event.n };
            };"#,
        );

        let pool = ProcessPool::new();
        let env = HashMap::new();
        let ctx = serde_json::json!({});

        // 5 rapid sequential invocations — worker should stay alive
        for i in 1..=5 {
            let result = pool
                .invoke(
                    "rapid_fn",
                    &crate::config::Runtime::Nodejs20,
                    "rapid.handler",
                    tmp.path(),
                    &env,
                    &serde_json::json!({"n": i}),
                    &ctx,
                )
                .await
                .unwrap();
            assert_eq!(result["body"], i.to_string());
        }

        // Only 1 worker should exist
        let workers = pool.workers.lock().await;
        assert_eq!(workers.len(), 1);
    }
}
