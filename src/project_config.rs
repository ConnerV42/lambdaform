//! Project-level configuration via `lambdaform.yaml`
//!
//! Allows overriding Terraform-parsed settings without modifying .tf files.
//! Useful for local dev customizations (ports, env vars, source paths, etc.)

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::plugin::PluginEntry;

/// Top-level project config (lambdaform.yaml)
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]

pub struct ProjectConfig {
    /// Default port for the local server
    pub port: Option<u16>,

    /// Enable/disable hot reload (default: true)
    pub watch: Option<bool>,

    /// Global environment variables applied to all functions
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Per-function overrides (keyed by resource name or function_name)
    #[serde(default)]
    pub functions: HashMap<String, FunctionOverride>,

    /// Source directory base path (relative to config file location)
    pub source_dir: Option<PathBuf>,

    /// CORS configuration for the local server
    pub cors: Option<CorsConfig>,

    /// Debug configuration
    pub debug: Option<DebugConfig>,

    /// Enable structured JSON log output (default: false)
    pub json_log: Option<bool>,

    /// Per-gateway overrides (keyed by resource name)
    #[serde(default)]
    pub gateways: HashMap<String, GatewayOverride>,

    /// Plugin configurations
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

/// Per-gateway overrides
#[derive(Debug, Clone, Deserialize, Default)]

pub struct GatewayOverride {
    /// Override the port for this gateway
    pub port: Option<u16>,
}

/// Debug configuration
#[derive(Debug, Clone, Deserialize)]

pub struct DebugConfig {
    /// Enable Node.js inspector (default: false)
    #[serde(default)]
    pub nodejs: bool,

    /// Enable Python debugpy (default: false)
    #[serde(default)]
    pub python: bool,

    /// Inspector port for Node.js (default: 9229)
    #[serde(default = "default_debug_port")]
    pub port: u16,

    /// Debug port for Python/debugpy (default: 5678)
    #[serde(default = "default_python_debug_port")]
    pub python_port: u16,

    /// Break on first line (default: true, uses --inspect-brk)
    #[serde(default = "default_true")]
    pub break_on_start: bool,
}

fn default_debug_port() -> u16 {
    9229
}
fn default_python_debug_port() -> u16 {
    5678
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            nodejs: false,
            python: false,
            port: default_debug_port(),
            python_port: default_python_debug_port(),
            break_on_start: default_true(),
        }
    }
}
fn default_true() -> bool {
    true
}

/// CORS configuration
#[derive(Debug, Clone, Deserialize)]

pub struct CorsConfig {
    /// Allowed origins (default: ["*"])
    #[serde(default = "default_origins")]
    pub allow_origins: Vec<String>,

    /// Allowed HTTP methods (default: all standard methods)
    #[serde(default)]
    pub allow_methods: Vec<String>,

    /// Allowed headers (default: ["*"])
    #[serde(default)]
    pub allow_headers: Vec<String>,

    /// Headers to expose to the browser
    #[serde(default)]
    pub expose_headers: Vec<String>,

    /// Whether to allow credentials (default: false)
    #[serde(default)]
    pub allow_credentials: bool,

    /// Max age for preflight cache in seconds (default: 86400)
    pub max_age: Option<u64>,
}

fn default_origins() -> Vec<String> {
    vec!["*".to_string()]
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allow_origins: vec!["*".to_string()],
            allow_methods: vec![],
            allow_headers: vec![],
            expose_headers: vec![],
            allow_credentials: false,
            max_age: None,
        }
    }
}

/// Per-function overrides
#[derive(Debug, Clone, Deserialize, Default)]

pub struct FunctionOverride {
    /// Override the handler
    pub handler: Option<String>,

    /// Override source path
    pub source_path: Option<PathBuf>,

    /// Additional environment variables (merged with global + Terraform)
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Override timeout
    pub timeout: Option<u32>,

    /// Override memory size
    pub memory_size: Option<u32>,
}

impl ProjectConfig {
    /// Load from a directory, looking for lambdaform.yaml or lambdaform.yml
    pub fn load(dir: &Path) -> anyhow::Result<Option<Self>> {
        let yaml_path = dir.join("lambdaform.yaml");
        let yml_path = dir.join("lambdaform.yml");

        let path = if yaml_path.exists() {
            yaml_path
        } else if yml_path.exists() {
            yml_path
        } else {
            return Ok(None);
        };

        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;

        let config: ProjectConfig = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Invalid config in {}: {}", path.display(), e))?;

        tracing::info!("📄 Loaded config from {}", path.display());
        Ok(Some(config))
    }

    /// Apply overrides to a parsed LambdaformConfig
    pub fn apply(&self, config: &mut crate::config::LambdaformConfig) {
        // Warn about unmatched function overrides in lambdaform.yaml
        for yaml_name in self.functions.keys() {
            let matched = config
                .functions
                .iter()
                .any(|f| f.resource_name == *yaml_name || f.function_name == *yaml_name);
            if !matched {
                tracing::warn!(
                    "⚠️  lambdaform.yaml defines function '{}' but no matching Terraform function found. \
                     Check that the name matches either the resource name or the resolved function_name.",
                    yaml_name
                );
            }
        }

        for func in &mut config.functions {
            // Apply global env vars first (Terraform values take precedence, then global, then per-function)
            for (k, v) in &self.environment {
                func.environment
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }

            // Find per-function override by resource_name or function_name
            let override_cfg = self
                .functions
                .get(&func.resource_name)
                .or_else(|| self.functions.get(&func.function_name));

            if let Some(ovr) = override_cfg {
                if let Some(handler) = &ovr.handler {
                    func.handler = handler.clone();
                }
                if let Some(source_path) = &ovr.source_path {
                    func.source_path = Some(source_path.clone());
                }
                if let Some(timeout) = ovr.timeout {
                    func.timeout = timeout;
                }
                if let Some(memory_size) = ovr.memory_size {
                    func.memory_size = memory_size;
                }
                // Per-function env vars override everything
                for (k, v) in &ovr.environment {
                    func.environment.insert(k.clone(), v.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
port: 8080
watch: false
environment:
  STAGE: local
  LOG_LEVEL: debug
functions:
  my_api:
    handler: src/handler.main
    source_path: ./lambdas/my-api
    environment:
      DB_HOST: localhost
    timeout: 30
    memory_size: 256
"#;
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.port, Some(8080));
        assert_eq!(config.watch, Some(false));
        assert_eq!(config.environment.get("STAGE").unwrap(), "local");
        let func = config.functions.get("my_api").unwrap();
        assert_eq!(func.handler.as_deref(), Some("src/handler.main"));
        assert_eq!(func.timeout, Some(30));
    }

    #[test]
    fn test_parse_minimal_config() {
        let yaml = "port: 4000\n";
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.port, Some(4000));
        assert!(config.functions.is_empty());
    }

    #[test]
    fn test_parse_json_log_config() {
        let yaml = "port: 3000\njson_log: true\n";
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.json_log, Some(true));
    }

    #[test]
    fn test_apply_global_env() {
        let yaml = r#"
environment:
  STAGE: local
"#;
        let proj: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let mut config = crate::config::LambdaformConfig {
            functions: vec![crate::config::LambdaConfig {
                resource_name: "test".into(),
                function_name: "test-fn".into(),
                handler: "index.handler".into(),
                runtime: crate::config::Runtime::Nodejs20,
                source_path: None,
                filename_ref: None,
                environment: HashMap::new(),
                timeout: 3,
                memory_size: 128,
                layers: vec![],
            }],
            gateways: vec![],
            layers: vec![],
            state_machines: vec![],
            dynamodb_tables: vec![],
            sqs_queues: vec![],
            sns_topics: vec![],
            event_source_mappings: vec![],
            archive_files: vec![],
            detected_cors: None,
        };
        proj.apply(&mut config);
        assert_eq!(
            config.functions[0].environment.get("STAGE").unwrap(),
            "local"
        );
    }
}
