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
    /// Maximum number of history entries to keep (rotate on startup)
    const MAX_ENTRIES: usize = 1000;

    /// Create a new recorder, writing to `.lambdaform/history.jsonl` in the given dir.
    /// Rotates the history file on startup if it exceeds MAX_ENTRIES.
    pub fn new(project_dir: &Path) -> std::io::Result<Self> {
        let dir = project_dir.join(".lambdaform");
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join("history.jsonl");

        // Rotate on startup: keep only the last MAX_ENTRIES
        if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
                if lines.len() > Self::MAX_ENTRIES {
                    let kept = &lines[lines.len() - Self::MAX_ENTRIES..];
                    let _ = std::fs::write(&file_path, kept.join("\n") + "\n");
                    tracing::info!(
                        "Rotated history: {} → {} entries",
                        lines.len(),
                        Self::MAX_ENTRIES
                    );
                }
            }
        }

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

    fn make_entry(status: u16) -> HistoryEntry {
        HistoryEntry {
            id: "t".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            method: "GET".to_string(),
            path: "/".to_string(),
            query: None,
            headers: None,
            body: None,
            function: "fn".to_string(),
            status,
            response_body: None,
            duration_ms: 10,
            port: 3000,
        }
    }

    #[test]
    fn test_format_entry_status_icons() {
        assert!(format_entry(&make_entry(200), 0).contains("✅"));
        assert!(format_entry(&make_entry(301), 0).contains("↪️"));
        assert!(format_entry(&make_entry(404), 0).contains("⚠️"));
        assert!(format_entry(&make_entry(500), 0).contains("❌"));
    }

    #[test]
    fn test_format_entry_no_query() {
        let formatted = format_entry(&make_entry(200), 5);
        assert!(formatted.contains("[5]"));
        assert!(!formatted.contains("?"));
    }

    #[test]
    fn test_load_history_skips_malformed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.jsonl");
        let good = serde_json::to_string(&make_entry(200)).unwrap();
        std::fs::write(&path, format!("{}\nnot json\n{}\n", good, good)).unwrap();
        let entries = load_history(&path).unwrap();
        assert_eq!(entries.len(), 2); // skips the bad line
    }

    #[test]
    fn test_load_history_missing_file() {
        let result = load_history(Path::new("/nonexistent/history.jsonl"));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rotation_on_startup() {
        let dir = TempDir::new().unwrap();
        let lf_dir = dir.path().join(".lambdaform");
        std::fs::create_dir_all(&lf_dir).unwrap();
        let path = lf_dir.join("history.jsonl");

        // Write 1050 lines (over MAX_ENTRIES of 1000)
        let entry = make_entry(200);
        let line = serde_json::to_string(&entry).unwrap();
        let content: String = (0..1050).map(|_| format!("{}\n", line)).collect();
        std::fs::write(&path, &content).unwrap();

        // Creating recorder triggers rotation
        let _recorder = HistoryRecorder::new(dir.path()).unwrap();

        let loaded = load_history(&path).unwrap();
        assert_eq!(loaded.len(), 1000);
    }

    #[tokio::test]
    async fn test_multiple_concurrent_records() {
        let dir = TempDir::new().unwrap();
        let recorder = HistoryRecorder::new(dir.path()).unwrap();

        // Record several entries rapidly
        for i in 0..10 {
            let mut entry = make_entry(200);
            entry.id = format!("concurrent-{}", i);
            entry.path = format!("/api/item/{}", i);
            recorder.record(entry).await;
        }

        assert_eq!(recorder.count().await, 10);
        let loaded = load_history(recorder.file_path()).unwrap();
        assert_eq!(loaded.len(), 10);
        // Verify ordering preserved
        assert_eq!(loaded[0].id, "concurrent-0");
        assert_eq!(loaded[9].id, "concurrent-9");
    }

    #[test]
    fn test_format_entry_redirect_icon() {
        // 3xx should show redirect icon
        let formatted = format_entry(&make_entry(302), 0);
        assert!(formatted.contains("↪️"));
        assert!(formatted.contains("302"));
    }

    #[test]
    fn test_format_entry_with_headers() {
        let mut entry = make_entry(200);
        entry.headers = Some(HashMap::from([
            ("content-type".to_string(), "application/json".to_string()),
            ("authorization".to_string(), "Bearer xxx".to_string()),
        ]));
        // Headers should serialize without error
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.headers.unwrap().len(), 2);
    }

    #[test]
    fn test_load_history_whitespace_only_lines() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ws.jsonl");
        let good = serde_json::to_string(&make_entry(200)).unwrap();
        std::fs::write(&path, format!("{}\n  \n\n{}\n", good, good)).unwrap();
        let entries = load_history(&path).unwrap();
        assert_eq!(entries.len(), 2); // blank lines skipped
    }

    #[tokio::test]
    async fn test_recorder_creates_dotlambdaform_dir() {
        let dir = TempDir::new().unwrap();
        let lf_dir = dir.path().join(".lambdaform");
        assert!(!lf_dir.exists());
        let _recorder = HistoryRecorder::new(dir.path()).unwrap();
        assert!(lf_dir.exists());
    }

    #[tokio::test]
    async fn test_rotation_keeps_exactly_max() {
        let dir = TempDir::new().unwrap();
        let lf_dir = dir.path().join(".lambdaform");
        std::fs::create_dir_all(&lf_dir).unwrap();
        let path = lf_dir.join("history.jsonl");

        // Write exactly 1000 — should NOT rotate
        let entry = make_entry(200);
        let line = serde_json::to_string(&entry).unwrap();
        let content: String = (0..1000).map(|_| format!("{}\n", line)).collect();
        std::fs::write(&path, &content).unwrap();

        let _recorder = HistoryRecorder::new(dir.path()).unwrap();
        let loaded = load_history(&path).unwrap();
        assert_eq!(loaded.len(), 1000); // no rotation needed
    }

    #[test]
    fn test_entry_serialization_roundtrip() {
        let entry = HistoryEntry {
            id: "rt-1".to_string(),
            timestamp: "2026-02-22T00:00:00Z".to_string(),
            method: "PUT".to_string(),
            path: "/items/42".to_string(),
            query: Some(HashMap::from([("v".to_string(), "2".to_string())])),
            headers: Some(HashMap::from([(
                "content-type".to_string(),
                "application/json".to_string(),
            )])),
            body: Some("{\"name\":\"updated\"}".to_string()),
            function: "update_fn".to_string(),
            status: 204,
            response_body: None,
            duration_ms: 88,
            port: 4000,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "rt-1");
        assert_eq!(deserialized.method, "PUT");
        assert_eq!(deserialized.status, 204);
        assert_eq!(deserialized.query.unwrap().get("v").unwrap(), "2");
    }
}
