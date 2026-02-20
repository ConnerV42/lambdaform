//! File watcher for hot reload

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::sync::mpsc;
use std::time::Duration;

/// Watch configuration for hot reload
pub struct WatchConfig {
    /// Directories to watch
    pub watch_paths: Vec<std::path::PathBuf>,

    /// File patterns to ignore
    pub ignore_patterns: Vec<String>,

    /// Debounce duration
    pub debounce_ms: u64,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec![],
            ignore_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "__pycache__".to_string(),
                ".terraform".to_string(),
            ],
            debounce_ms: 100,
        }
    }
}

/// File change event
#[derive(Debug)]
pub enum FileChange {
    /// Source file changed (need to reload function)
    Source(std::path::PathBuf),

    /// Terraform file changed (need to reload config)
    Terraform(std::path::PathBuf),
}

/// Handle returned by start_watching — must be kept alive
pub struct WatchHandle {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

/// Start watching for file changes. Returns a handle that must be kept alive.
pub fn start_watching(
    config: WatchConfig,
    callback: impl Fn(FileChange) + Send + 'static,
) -> anyhow::Result<WatchHandle> {
    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_millis(config.debounce_ms), tx)?;

    for path in &config.watch_paths {
        debouncer.watcher().watch(path, RecursiveMode::Recursive)?;
        tracing::debug!("Watching: {}", path.display());
    }

    // Process events in a background thread
    std::thread::spawn(move || {
        for result in rx {
            match result {
                Ok(events) => {
                    for event in events {
                        if let Some(change) = process_event(&event, &config.ignore_patterns) {
                            callback(change);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Watch error: {:?}", e);
                }
            }
        }
    });

    Ok(WatchHandle {
        _debouncer: debouncer,
    })
}

/// Process a debounced event into a FileChange
fn process_event(event: &DebouncedEvent, ignore_patterns: &[String]) -> Option<FileChange> {
    let path = &event.path;

    // Check ignore patterns
    let path_str = path.to_string_lossy();
    for pattern in ignore_patterns {
        if path_str.contains(pattern) {
            return None;
        }
    }

    // Determine change type
    let extension = path.extension()?.to_str()?;

    match extension {
        "tf" | "tfvars" => Some(FileChange::Terraform(path.clone())),
        "yaml" | "yml"
            if path
                .file_name()
                .map(|f| f.to_string_lossy().contains("lambdaform"))
                .unwrap_or(false) =>
        {
            Some(FileChange::Terraform(path.clone()))
        }
        "js" | "ts" | "mjs" | "cjs" | "py" | "go" | "rs" => Some(FileChange::Source(path.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ignore_patterns() {
        let event = DebouncedEvent {
            path: std::path::PathBuf::from("/project/node_modules/test.js"),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let ignore = vec!["node_modules".to_string()];
        assert!(process_event(&event, &ignore).is_none());
    }

    #[test]
    fn test_tf_detection() {
        let event = DebouncedEvent {
            path: std::path::PathBuf::from("/project/main.tf"),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let change = process_event(&event, &[]);
        assert!(matches!(change, Some(FileChange::Terraform(_))));
    }

    #[test]
    fn test_lambdaform_yaml_detection() {
        let event = DebouncedEvent {
            path: std::path::PathBuf::from("/project/lambdaform.yaml"),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let change = process_event(&event, &[]);
        assert!(matches!(change, Some(FileChange::Terraform(_))));
    }

    #[test]
    fn test_lambdaform_yml_detection() {
        let event = DebouncedEvent {
            path: std::path::PathBuf::from("/project/lambdaform.yml"),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let change = process_event(&event, &[]);
        assert!(matches!(change, Some(FileChange::Terraform(_))));
    }

    #[test]
    fn test_random_yaml_not_detected() {
        let event = DebouncedEvent {
            path: std::path::PathBuf::from("/project/config.yaml"),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let change = process_event(&event, &[]);
        assert!(change.is_none());
    }

    #[test]
    fn test_source_file_detection() {
        for ext in &["js", "ts", "py", "go", "rs"] {
            let event = DebouncedEvent {
                path: std::path::PathBuf::from(format!("/project/handler.{ext}")),
                kind: notify_debouncer_mini::DebouncedEventKind::Any,
            };
            let change = process_event(&event, &[]);
            assert!(
                matches!(change, Some(FileChange::Source(_))),
                "Expected Source for .{ext}"
            );
        }
    }

    #[test]
    fn test_tfvars_detection() {
        let event = DebouncedEvent {
            path: std::path::PathBuf::from("/project/dev.tfvars"),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let change = process_event(&event, &[]);
        assert!(matches!(change, Some(FileChange::Terraform(_))));
    }
}
