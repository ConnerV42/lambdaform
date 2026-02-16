//! Request history recording and replay
//!
//! Records HTTP requests/responses during `lambdaform start` to `.lambdaform/history.jsonl`.
//! Supports replaying recorded requests via `lambdaform replay`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A recorded HTTP request/response pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unique request ID
    pub id: String,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// HTTP method
    pub method: String,
    /// Request path
    pub path: String,
    /// Query parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<HashMap<String, String>>,
    /// Request headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// Request body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Matched function name
    pub function: String,
    /// Response status code
    pub status: u16,
    /// Response body (truncated to 10KB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Gateway port this request was served on
    pub port: u16,
}

/// Thread-safe history recorder that appends to a JSONL file
#[derive(Clone)]
pub struct HistoryRecorder {
    file_path: PathBuf,
    inner: Arc<Mutex<HistoryRecorderInner>>,
}

struct HistoryRecorderInner {
    count: usize,
}

impl HistoryRecorder {
    /// Create a new recorder, writing to `.lambdaform/history.jsonl` in the given dir
    pub fn new(project_dir: &Path) -> std::io::Result<Self> {
        let dir = project_dir.join(".lambdaform");
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join("history.jsonl");

        Ok(Self {
            file_path,
            inner: Arc::new(Mutex::new(HistoryRecorderInner { count: 0 })),
        })
    }

    /// Record a request/response entry (appends to JSONL file)
    pub async fn record(&self, entry: HistoryEntry) {
        let mut inner = self.inner.lock().await;
        inner.count += 1;

        // Append to file (best-effort, don't fail the request)
        if let Ok(line) = serde_json::to_string(&entry) {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
            {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    /// Get the number of recorded entries this session
    pub async fn count(&self) -> usize {
        self.inner.lock().await.count
    }

    /// Get the history file path
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

/// Load history entries from a JSONL file
pub fn load_history(path: &Path) -> anyhow::Result<Vec<HistoryEntry>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read history file {}: {}", path.display(), e))?;

    let mut entries = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<HistoryEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                tracing::warn!("Skipping malformed history entry on line {}: {}", i + 1, e);
            }
        }
    }
    Ok(entries)
}

/// Format a history entry for display
pub fn format_entry(entry: &HistoryEntry, index: usize) -> String {
    let status_icon = if entry.status < 300 {
        "✅"
    } else if entry.status < 400 {
        "↪️"
    } else if entry.status < 500 {
        "⚠️"
    } else {
        "❌"
    };

    let query_str = entry
        .query
        .as_ref()
        .filter(|q| !q.is_empty())
        .map(|q| {
            format!(
                "?{}",
                q.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&")
            )
        })
        .unwrap_or_default();

    format!(
        "[{}] {} {} {} {}{} → {} ({}ms)",
        index,
        entry.timestamp,
        status_icon,
        entry.method,
        entry.path,
        query_str,
        entry.status,
        entry.duration_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_record_and_load() {
        let dir = TempDir::new().unwrap();
        let recorder = HistoryRecorder::new(dir.path()).unwrap();

        let entry = HistoryEntry {
            id: "test-1".to_string(),
            timestamp: "2026-02-15T21:00:00Z".to_string(),
            method: "GET".to_string(),
            path: "/api/items".to_string(),
            query: None,
            headers: None,
            body: None,
            function: "my_handler".to_string(),
            status: 200,
            response_body: Some("{\"ok\":true}".to_string()),
            duration_ms: 42,
            port: 3000,
        };

        recorder.record(entry.clone()).await;
        recorder.record(entry).await;
        assert_eq!(recorder.count().await, 2);

        let loaded = load_history(recorder.file_path()).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].method, "GET");
        assert_eq!(loaded[0].function, "my_handler");
    }

    #[tokio::test]
    async fn test_load_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.jsonl");
        std::fs::write(&path, "").unwrap();
        let entries = load_history(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_format_entry() {
        let entry = HistoryEntry {
            id: "test-1".to_string(),
            timestamp: "2026-02-15T21:00:00Z".to_string(),
            method: "POST".to_string(),
            path: "/api/items".to_string(),
            query: Some(HashMap::from([("page".to_string(), "1".to_string())])),
            headers: None,
            body: Some("{\"name\":\"test\"}".to_string()),
            function: "create_item".to_string(),
            status: 201,
            response_body: None,
            duration_ms: 150,
            port: 3000,
        };

        let formatted = format_entry(&entry, 0);
        assert!(formatted.contains("POST"));
        assert!(formatted.contains("/api/items"));
        assert!(formatted.contains("201"));
        assert!(formatted.contains("150ms"));
    }
}
