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
                architecture: crate::config::Architecture::default(),
            }],
            gateways: vec![],
            layers: vec![],
            state_machines: vec![],
            dynamodb_tables: vec![],
            sqs_queues: vec![],
            sns_topics: vec![],
            event_source_mappings: vec![],
            archive_files: vec![],
            function_urls: vec![],
            detected_cors: None,
        };
        proj.apply(&mut config);
        assert_eq!(
            config.functions[0].environment.get("STAGE").unwrap(),
            "local"
        );
    }

    #[test]
    fn test_parse_empty_config() {
        let yaml = "{}";
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.port, None);
        assert_eq!(config.watch, None);
        assert!(config.environment.is_empty());
        assert!(config.functions.is_empty());
        assert!(config.gateways.is_empty());
        assert!(config.plugins.is_empty());
    }

    #[test]
    fn test_debug_config_defaults() {
        let yaml = "debug:\n  nodejs: true\n";
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let debug = config.debug.unwrap();
        assert!(debug.nodejs);
        assert!(!debug.python);
        assert_eq!(debug.port, 9229);
        assert_eq!(debug.python_port, 5678);
        assert!(debug.break_on_start);
    }

    #[test]
    fn test_debug_config_custom_ports() {
        let yaml = "debug:\n  nodejs: true\n  port: 9999\n  python: true\n  python_port: 6789\n  break_on_start: false\n";
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let debug = config.debug.unwrap();
        assert_eq!(debug.port, 9999);
        assert_eq!(debug.python_port, 6789);
        assert!(!debug.break_on_start);
    }

    #[test]
    fn test_gateway_override() {
        let yaml = "gateways:\n  my_api:\n    port: 9000\n";
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let gw = config.gateways.get("my_api").unwrap();
        assert_eq!(gw.port, Some(9000));
    }

    #[test]
    fn test_cors_config() {
        let yaml = r#"
cors:
  allow_origins:
    - "http://localhost:3000"
    - "https://example.com"
  max_age: 7200
"#;
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let cors = config.cors.unwrap();
        assert_eq!(cors.allow_origins.len(), 2);
        assert_eq!(cors.allow_origins[0], "http://localhost:3000");
        assert_eq!(cors.max_age, Some(7200));
    }

    #[test]
    fn test_function_override_all_fields() {
        let yaml = r#"
functions:
  my_fn:
    handler: new.handler
    source_path: ./override/path
    timeout: 60
    memory_size: 512
    environment:
      KEY: value
"#;
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let f = config.functions.get("my_fn").unwrap();
        assert_eq!(f.handler.as_deref(), Some("new.handler"));
        assert_eq!(f.timeout, Some(60));
        assert_eq!(f.memory_size, Some(512));
        assert_eq!(f.environment.get("KEY").unwrap(), "value");
    }

    #[test]
    fn test_plugins_config() {
        let yaml = r#"
plugins:
  - name: s3-local
    path: ./plugins/s3-local
"#;
        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.plugins.len(), 1);
        assert_eq!(config.plugins[0].name, "s3-local");
    }

    fn make_lambda(resource: &str, function: &str) -> crate::config::LambdaConfig {
        crate::config::LambdaConfig {
            resource_name: resource.into(),
            function_name: function.into(),
            handler: "index.handler".into(),
            runtime: crate::config::Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 3,
            memory_size: 128,
            layers: vec![],
            architecture: crate::config::Architecture::default(),
        }
    }

    fn make_config(funcs: Vec<crate::config::LambdaConfig>) -> crate::config::LambdaformConfig {
        crate::config::LambdaformConfig {
            functions: funcs,
            ..Default::default()
        }
    }

    #[test]
    fn test_apply_per_function_override_by_resource_name() {
        let yaml = r#"
functions:
  api_handler:
    timeout: 60
    memory_size: 512
    handler: src/main.handle
"#;
        let proj: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let mut config = make_config(vec![make_lambda("api_handler", "my-api")]);
        proj.apply(&mut config);
        assert_eq!(config.functions[0].timeout, 60);
        assert_eq!(config.functions[0].memory_size, 512);
        assert_eq!(config.functions[0].handler, "src/main.handle");
    }

    #[test]
    fn test_apply_per_function_override_by_function_name() {
        let yaml = r#"
functions:
  my-api:
    timeout: 120
"#;
        let proj: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let mut config = make_config(vec![make_lambda("api_handler", "my-api")]);
        proj.apply(&mut config);
        assert_eq!(config.functions[0].timeout, 120);
    }

    #[test]
    fn test_apply_env_precedence() {
        // Terraform env > global yaml env; per-function yaml env overrides all
        let yaml = r#"
environment:
  GLOBAL_VAR: global_val
  SHARED: from_global
functions:
  fn1:
    environment:
      SHARED: from_per_function
      FN_VAR: fn_val
"#;
        let proj: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let mut func = make_lambda("fn1", "fn1-name");
        func.environment.insert("TF_VAR".into(), "tf_val".into());
        func.environment.insert("SHARED".into(), "from_tf".into());
        let mut config = make_config(vec![func]);
        proj.apply(&mut config);
        let env = &config.functions[0].environment;
        // Terraform value kept (global uses entry().or_insert)
        assert_eq!(env["TF_VAR"], "tf_val");
        assert_eq!(env["GLOBAL_VAR"], "global_val");
        // Per-function override wins over everything (uses insert)
        assert_eq!(env["SHARED"], "from_per_function");
        assert_eq!(env["FN_VAR"], "fn_val");
    }

    #[test]
    fn test_apply_source_path_override() {
        let yaml = r#"
functions:
  worker:
    source_path: ./custom/path
"#;
        let proj: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let mut config = make_config(vec![make_lambda("worker", "worker-fn")]);
        proj.apply(&mut config);
        assert_eq!(
            config.functions[0].source_path,
            Some(PathBuf::from("./custom/path"))
        );
    }

    #[test]
    fn test_apply_no_overrides_leaves_unchanged() {
        let yaml = "{}";
        let proj: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let mut config = make_config(vec![make_lambda("fn1", "fn1-name")]);
        config.functions[0].timeout = 30;
        config.functions[0].memory_size = 256;
        proj.apply(&mut config);
        assert_eq!(config.functions[0].timeout, 30);
        assert_eq!(config.functions[0].memory_size, 256);
    }

    #[test]
    fn test_apply_multiple_functions() {
        let yaml = r#"
environment:
  STAGE: dev
functions:
  fn_a:
    timeout: 10
  fn_b:
    timeout: 20
"#;
        let proj: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        let mut config = make_config(vec![
            make_lambda("fn_a", "func-a"),
            make_lambda("fn_b", "func-b"),
            make_lambda("fn_c", "func-c"),
        ]);
        proj.apply(&mut config);
        assert_eq!(config.functions[0].timeout, 10);
        assert_eq!(config.functions[1].timeout, 20);
        assert_eq!(config.functions[2].timeout, 3); // unchanged
                                                    // All get global env
        for f in &config.functions {
            assert_eq!(f.environment["STAGE"], "dev");
        }
    }

    #[test]
    fn test_load_nonexistent_dir() {
        let result = ProjectConfig::load(Path::new("/nonexistent/dir"));
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_yaml_extension() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("lambdaform.yaml"), "port: 5000\n").unwrap();
        let config = ProjectConfig::load(dir.path()).unwrap().unwrap();
        assert_eq!(config.port, Some(5000));
    }

    #[test]
    fn test_load_yml_extension() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("lambdaform.yml"), "port: 6000\n").unwrap();
        let config = ProjectConfig::load(dir.path()).unwrap().unwrap();
        assert_eq!(config.port, Some(6000));
    }

    #[test]
    fn test_load_yaml_preferred_over_yml() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("lambdaform.yaml"), "port: 7000\n").unwrap();
        std::fs::write(dir.path().join("lambdaform.yml"), "port: 8000\n").unwrap();
        let config = ProjectConfig::load(dir.path()).unwrap().unwrap();
        assert_eq!(config.port, Some(7000)); // .yaml wins
    }

    #[test]
    fn test_load_invalid_yaml() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("lambdaform.yaml"), "port: [invalid").unwrap();
        let result = ProjectConfig::load(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_cors_defaults() {
        let cors = CorsConfig::default();
        assert_eq!(cors.allow_origins, vec!["*"]);
        assert!(!cors.allow_credentials);
        assert!(cors.max_age.is_none());
    }

    #[test]
    fn test_unknown_fields_allowed() {
        // deny_unknown_fields was removed (M4) — verify unknown keys don't error
        let yaml = r#"
port: 3000
future_field: whatever
nested:
  unknown: true
"#;
        let result: Result<ProjectConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_ok());
    }
}
