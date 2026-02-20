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
