//! Plugin architecture for custom resource handlers
//!
//! Plugins are external executables that communicate with Lambdaform via JSON
//! over stdin/stdout. This enables users to extend Lambdaform with custom
//! resource handlers (e.g., S3, Cognito, custom services) without modifying
//! the core codebase.
//!
//! ## Plugin Protocol
//!
//! Lambdaform sends a JSON request to the plugin's stdin and reads a JSON
//! response from stdout. Each request has a `kind` field indicating the hook:
//!
//! - `describe`: Plugin returns its capabilities (called once at startup)
//! - `on_resource`: Called when Terraform resource types matching the plugin's
//!   `resource_types` are encountered during parsing
//! - `on_request`: Called before a Lambda function is invoked (can modify the event)
//! - `on_response`: Called after a Lambda function returns (can modify the response)
//!
//! ## Plugin Discovery
//!
//! Plugins are configured in `lambdaform.yaml`:
//!
//! ```yaml
//! plugins:
//!   - name: s3-local
//!     path: ./plugins/s3-local.py
//!     config:
//!       data_dir: /tmp/s3-local
//!   - name: my-plugin
//!     path: /usr/local/bin/my-lambdaform-plugin
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;

/// Plugin configuration from lambdaform.yaml
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginEntry {
    /// Plugin name (used in logs and error messages)
    pub name: String,

    /// Path to the plugin executable (absolute or relative to project root)
    pub path: String,

    /// Optional configuration passed to the plugin
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

/// A loaded and validated plugin
#[derive(Debug, Clone)]
pub struct Plugin {
    /// Plugin name
    pub name: String,

    /// Resolved absolute path to the executable
    pub executable: PathBuf,

    /// Plugin configuration
    pub config: HashMap<String, serde_json::Value>,

    /// Capabilities reported by the plugin
    pub capabilities: PluginCapabilities,
}

/// Capabilities reported by a plugin during the `describe` handshake
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PluginCapabilities {
    /// Plugin version
    #[serde(default)]
    pub version: String,

    /// Terraform resource types this plugin handles (e.g., ["aws_s3_bucket", "aws_s3_object"])
    #[serde(default)]
    pub resource_types: Vec<String>,

    /// Whether this plugin wants to intercept requests (on_request hook)
    #[serde(default)]
    pub intercept_requests: bool,

    /// Whether this plugin wants to intercept responses (on_response hook)
    #[serde(default)]
    pub intercept_responses: bool,

    /// Human-readable description
    #[serde(default)]
    pub description: String,
}

/// Request sent to a plugin via stdin
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum PluginRequest {
    /// Ask the plugin to describe its capabilities
    #[serde(rename = "describe")]
    Describe {
        /// Plugin config from lambdaform.yaml
        config: HashMap<String, serde_json::Value>,
    },

    /// A Terraform resource was parsed that matches the plugin's resource_types
    #[serde(rename = "on_resource")]
    OnResource {
        /// The Terraform resource type (e.g., "aws_s3_bucket")
        resource_type: String,
        /// The resource name (e.g., "my_bucket")
        resource_name: String,
        /// The resource attributes as parsed from HCL
        attributes: serde_json::Value,
        /// Plugin config
        config: HashMap<String, serde_json::Value>,
    },

    /// A request is about to be routed to a Lambda function
    #[serde(rename = "on_request")]
    OnRequest {
        /// HTTP method
        method: String,
        /// Request path
        path: String,
        /// The Lambda event that will be sent to the function
        event: serde_json::Value,
        /// Target function name
        function_name: String,
        /// Plugin config
        config: HashMap<String, serde_json::Value>,
    },

    /// A Lambda function has returned a response
    #[serde(rename = "on_response")]
    OnResponse {
        /// HTTP method of the original request
        method: String,
        /// Request path
        path: String,
        /// The Lambda function's response
        response: serde_json::Value,
        /// Function name that handled it
        function_name: String,
        /// Plugin config
        config: HashMap<String, serde_json::Value>,
    },
}

/// Response from a plugin via stdout
#[derive(Debug, Deserialize)]
pub struct PluginResponse {
    /// Whether the plugin handled this successfully
    #[serde(default = "default_true")]
    pub ok: bool,

    /// Error message (if ok is false)
    pub error: Option<String>,

    /// For `describe`: the plugin's capabilities
    pub capabilities: Option<PluginCapabilities>,

    /// For `on_resource`: any side effects the plugin wants to register
    pub side_effects: Option<Vec<PluginSideEffect>>,

    /// For `on_request`: optionally modified event (None = pass through unchanged)
    pub event: Option<serde_json::Value>,

    /// For `on_response`: optionally modified response (None = pass through unchanged)
    pub response: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}

/// Side effects a plugin can register when handling a resource
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum PluginSideEffect {
    /// Register an environment variable for Lambda functions
    #[serde(rename = "env_var")]
    EnvVar {
        /// Which functions to apply to (empty = all)
        #[serde(default)]
        functions: Vec<String>,
        key: String,
        value: String,
    },

    /// Register a local endpoint the plugin serves
    #[serde(rename = "endpoint")]
    Endpoint {
        /// Service name (e.g., "s3", "dynamodb")
        service: String,
        /// Local URL the plugin serves
        url: String,
    },

    /// Log a message
    #[serde(rename = "log")]
    Log { level: String, message: String },
}

/// Manages all loaded plugins
#[derive(Debug, Clone)]
pub struct PluginManager {
    plugins: Vec<Plugin>,
    /// Timeout for plugin invocations (seconds)
    timeout_secs: u64,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self {
            plugins: Vec::new(),
            timeout_secs: 10,
        }
    }
}

impl PluginManager {
    /// Create a new empty plugin manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Load and initialize plugins from config entries.
    /// Calls `describe` on each plugin to validate and discover capabilities.
    pub async fn load_plugins(entries: &[PluginEntry], project_dir: &Path) -> Result<Self> {
        let mut manager = Self::new();

        for entry in entries {
            match manager.load_plugin(entry, project_dir).await {
                Ok(plugin) => {
                    tracing::info!(
                        "🔌 Plugin '{}' loaded: {} resource types, intercept_requests={}, intercept_responses={}",
                        plugin.name,
                        plugin.capabilities.resource_types.len(),
                        plugin.capabilities.intercept_requests,
                        plugin.capabilities.intercept_responses,
                    );
                    if !plugin.capabilities.description.is_empty() {
                        tracing::info!("   ℹ️  {}", plugin.capabilities.description);
                    }
                    manager.plugins.push(plugin);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to load plugin '{}': {}", entry.name, e);
                    return Err(e).context(format!("Failed to load plugin '{}'", entry.name));
                }
            }
        }

        Ok(manager)
    }

    /// Load a single plugin: resolve path, run `describe`, validate
    async fn load_plugin(&self, entry: &PluginEntry, project_dir: &Path) -> Result<Plugin> {
        let executable = resolve_plugin_path(&entry.path, project_dir)?;

        // Verify the file exists and is executable
        if !executable.exists() {
            anyhow::bail!("Plugin executable not found: {}", executable.display());
        }

        // Send describe request
        let request = PluginRequest::Describe {
            config: entry.config.clone(),
        };
        let response = invoke_plugin(&executable, &request, self.timeout_secs)
            .await
            .context("Plugin 'describe' handshake failed")?;

        if !response.ok {
            anyhow::bail!(
                "Plugin returned error during describe: {}",
                response.error.unwrap_or_else(|| "unknown error".into())
            );
        }

        let capabilities = response.capabilities.unwrap_or_default();

        Ok(Plugin {
            name: entry.name.clone(),
            executable,
            config: entry.config.clone(),
            capabilities,
        })
    }

    /// Get all plugins that handle a given resource type
    pub fn plugins_for_resource(&self, resource_type: &str) -> Vec<&Plugin> {
        self.plugins
            .iter()
            .filter(|p| {
                p.capabilities
                    .resource_types
                    .iter()
                    .any(|rt| rt == resource_type)
            })
            .collect()
    }

    /// Get all plugins that want to intercept requests
    pub fn request_interceptors(&self) -> Vec<&Plugin> {
        self.plugins
            .iter()
            .filter(|p| p.capabilities.intercept_requests)
            .collect()
    }

    /// Get all plugins that want to intercept responses
    pub fn response_interceptors(&self) -> Vec<&Plugin> {
        self.plugins
            .iter()
            .filter(|p| p.capabilities.intercept_responses)
            .collect()
    }

    /// Notify plugins about a parsed Terraform resource.
    /// Returns accumulated side effects from all matching plugins.
    pub async fn on_resource(
        &self,
        resource_type: &str,
        resource_name: &str,
        attributes: serde_json::Value,
    ) -> Result<Vec<PluginSideEffect>> {
        let plugins = self.plugins_for_resource(resource_type);
        if plugins.is_empty() {
            return Ok(vec![]);
        }

        let mut all_effects = Vec::new();

        for plugin in plugins {
            let request = PluginRequest::OnResource {
                resource_type: resource_type.to_string(),
                resource_name: resource_name.to_string(),
                attributes: attributes.clone(),
                config: plugin.config.clone(),
            };

            match invoke_plugin(&plugin.executable, &request, self.timeout_secs).await {
                Ok(response) => {
                    if !response.ok {
                        tracing::warn!(
                            "⚠️ Plugin '{}' returned error for resource {}.{}: {}",
                            plugin.name,
                            resource_type,
                            resource_name,
                            response.error.unwrap_or_else(|| "unknown".into())
                        );
                        continue;
                    }
                    if let Some(effects) = response.side_effects {
                        for effect in &effects {
                            match effect {
                                PluginSideEffect::EnvVar { key, value, .. } => {
                                    tracing::debug!(
                                        "🔌 Plugin '{}' sets env {}={}",
                                        plugin.name,
                                        key,
                                        value
                                    );
                                }
                                PluginSideEffect::Endpoint { service, url } => {
                                    tracing::info!(
                                        "🔌 Plugin '{}' serves {} at {}",
                                        plugin.name,
                                        service,
                                        url
                                    );
                                }
                                PluginSideEffect::Log { level, message } => match level.as_str() {
                                    "error" => {
                                        tracing::error!("[plugin:{}] {}", plugin.name, message)
                                    }
                                    "warn" => {
                                        tracing::warn!("[plugin:{}] {}", plugin.name, message)
                                    }
                                    "debug" => {
                                        tracing::debug!("[plugin:{}] {}", plugin.name, message)
                                    }
                                    _ => tracing::info!("[plugin:{}] {}", plugin.name, message),
                                },
                            }
                        }
                        all_effects.extend(effects);
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "❌ Plugin '{}' failed on resource {}.{}: {}",
                        plugin.name,
                        resource_type,
                        resource_name,
                        e
                    );
                }
            }
        }

        Ok(all_effects)
    }

    /// Run on_request hooks. Returns the (possibly modified) event.
    pub async fn on_request(
        &self,
        method: &str,
        path: &str,
        event: serde_json::Value,
        function_name: &str,
    ) -> Result<serde_json::Value> {
        let mut current_event = event;

        for plugin in self.request_interceptors() {
            let request = PluginRequest::OnRequest {
                method: method.to_string(),
                path: path.to_string(),
                event: current_event.clone(),
                function_name: function_name.to_string(),
                config: plugin.config.clone(),
            };

            match invoke_plugin(&plugin.executable, &request, self.timeout_secs).await {
                Ok(response) => {
                    if !response.ok {
                        tracing::warn!(
                            "⚠️ Plugin '{}' on_request error: {}",
                            plugin.name,
                            response.error.unwrap_or_else(|| "unknown".into())
                        );
                        continue;
                    }
                    if let Some(modified_event) = response.event {
                        tracing::debug!(
                            "🔌 Plugin '{}' modified request event for {} {}",
                            plugin.name,
                            method,
                            path
                        );
                        current_event = modified_event;
                    }
                }
                Err(e) => {
                    tracing::error!("❌ Plugin '{}' on_request failed: {}", plugin.name, e);
                }
            }
        }

        Ok(current_event)
    }

    /// Run on_response hooks. Returns the (possibly modified) response.
    pub async fn on_response(
        &self,
        method: &str,
        path: &str,
        response: serde_json::Value,
        function_name: &str,
    ) -> Result<serde_json::Value> {
        let mut current_response = response;

        for plugin in self.response_interceptors() {
            let request = PluginRequest::OnResponse {
                method: method.to_string(),
                path: path.to_string(),
                response: current_response.clone(),
                function_name: function_name.to_string(),
                config: plugin.config.clone(),
            };

            match invoke_plugin(&plugin.executable, &request, self.timeout_secs).await {
                Ok(resp) => {
                    if !resp.ok {
                        tracing::warn!(
                            "⚠️ Plugin '{}' on_response error: {}",
                            plugin.name,
                            resp.error.unwrap_or_else(|| "unknown".into())
                        );
                        continue;
                    }
                    if let Some(modified_response) = resp.response {
                        tracing::debug!(
                            "🔌 Plugin '{}' modified response for {} {}",
                            plugin.name,
                            method,
                            path
                        );
                        current_response = modified_response;
                    }
                }
                Err(e) => {
                    tracing::error!("❌ Plugin '{}' on_response failed: {}", plugin.name, e);
                }
            }
        }

        Ok(current_response)
    }

    /// Check if any plugins are loaded
    pub fn has_plugins(&self) -> bool {
        !self.plugins.is_empty()
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// List loaded plugin names
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name.as_str()).collect()
    }
}

/// Resolve a plugin path (absolute or relative to project root)
fn resolve_plugin_path(path: &str, project_dir: &Path) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(project_dir.join(p))
    }
}

/// Invoke a plugin executable: serialize request to stdin, read response from stdout.
async fn invoke_plugin(
    executable: &Path,
    request: &PluginRequest,
    timeout_secs: u64,
) -> Result<PluginResponse> {
    let request_json =
        serde_json::to_string(request).context("Failed to serialize plugin request")?;

    let mut child = tokio::process::Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn plugin: {}", executable.display()))?;

    // Write request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request_json.as_bytes())
            .await
            .context("Failed to write to plugin stdin")?;
        stdin
            .shutdown()
            .await
            .context("Failed to close plugin stdin")?;
    }

    // Wait for completion with timeout
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Plugin timed out after {}s", timeout_secs))?
    .context("Failed to read plugin output")?;

    // Log stderr if any
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        for line in stderr.lines() {
            tracing::debug!("[plugin:stderr] {}", line);
        }
    }

    if !output.status.success() {
        anyhow::bail!(
            "Plugin exited with status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("Plugin stdout is not valid UTF-8")?;

    serde_json::from_str(&stdout).with_context(|| {
        format!(
            "Failed to parse plugin response: {}",
            stdout.chars().take(200).collect::<String>()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_request_serialization() {
        let req = PluginRequest::Describe {
            config: HashMap::new(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"kind\":\"describe\""));
    }

    #[test]
    fn test_plugin_response_deserialization() {
        let json = r#"{
            "ok": true,
            "capabilities": {
                "version": "1.0.0",
                "resource_types": ["aws_s3_bucket"],
                "intercept_requests": false,
                "intercept_responses": false,
                "description": "Local S3 emulator"
            }
        }"#;
        let resp: PluginResponse = serde_json::from_str(json).unwrap();
        assert!(resp.ok);
        let caps = resp.capabilities.unwrap();
        assert_eq!(caps.version, "1.0.0");
        assert_eq!(caps.resource_types, vec!["aws_s3_bucket"]);
        assert_eq!(caps.description, "Local S3 emulator");
    }

    #[test]
    fn test_plugin_response_with_side_effects() {
        let json = r#"{
            "ok": true,
            "side_effects": [
                {
                    "kind": "env_var",
                    "functions": [],
                    "key": "S3_ENDPOINT",
                    "value": "http://localhost:9000"
                },
                {
                    "kind": "endpoint",
                    "service": "s3",
                    "url": "http://localhost:9000"
                },
                {
                    "kind": "log",
                    "level": "info",
                    "message": "S3 bucket 'my-bucket' created"
                }
            ]
        }"#;
        let resp: PluginResponse = serde_json::from_str(json).unwrap();
        assert!(resp.ok);
        let effects = resp.side_effects.unwrap();
        assert_eq!(effects.len(), 3);
    }

    #[test]
    fn test_on_resource_request_serialization() {
        let req = PluginRequest::OnResource {
            resource_type: "aws_s3_bucket".to_string(),
            resource_name: "my_bucket".to_string(),
            attributes: serde_json::json!({"bucket": "test-bucket"}),
            config: HashMap::new(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"kind\":\"on_resource\""));
        assert!(json.contains("aws_s3_bucket"));
    }

    #[test]
    fn test_resolve_plugin_path_absolute() {
        let result = resolve_plugin_path("/usr/local/bin/plugin", Path::new("/project")).unwrap();
        assert_eq!(result, PathBuf::from("/usr/local/bin/plugin"));
    }

    #[test]
    fn test_resolve_plugin_path_relative() {
        let result = resolve_plugin_path("./plugins/my-plugin.py", Path::new("/project")).unwrap();
        assert_eq!(result, PathBuf::from("/project/./plugins/my-plugin.py"));
    }

    #[test]
    fn test_plugin_manager_empty() {
        let manager = PluginManager::new();
        assert!(!manager.has_plugins());
        assert_eq!(manager.plugin_count(), 0);
        assert!(manager.request_interceptors().is_empty());
        assert!(manager.response_interceptors().is_empty());
    }

    #[test]
    fn test_on_request_serialization() {
        let req = PluginRequest::OnRequest {
            method: "POST".to_string(),
            path: "/api/users".to_string(),
            event: serde_json::json!({"httpMethod": "POST"}),
            function_name: "create_user".to_string(),
            config: HashMap::new(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"kind\":\"on_request\""));
        assert!(json.contains("create_user"));
    }

    #[tokio::test]
    async fn test_invoke_real_plugin_describe() {
        // Find the test plugin relative to the workspace root
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let plugin_path = manifest_dir.join("tests/fixtures/test-plugin.py");
        if !plugin_path.exists() {
            // Skip if test plugin not present
            return;
        }

        let request = PluginRequest::Describe {
            config: HashMap::new(),
        };
        let response = invoke_plugin(&plugin_path, &request, 10).await.unwrap();
        assert!(response.ok);
        let caps = response.capabilities.unwrap();
        assert_eq!(caps.version, "0.1.0");
        assert!(caps.resource_types.contains(&"aws_s3_bucket".to_string()));
        assert!(caps.intercept_requests);
        assert!(caps.intercept_responses);
    }

    #[tokio::test]
    async fn test_invoke_real_plugin_on_resource() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let plugin_path = manifest_dir.join("tests/fixtures/test-plugin.py");
        if !plugin_path.exists() {
            return;
        }

        let request = PluginRequest::OnResource {
            resource_type: "aws_s3_bucket".to_string(),
            resource_name: "my_bucket".to_string(),
            attributes: serde_json::json!({"bucket": "test-bucket"}),
            config: HashMap::new(),
        };
        let response = invoke_plugin(&plugin_path, &request, 10).await.unwrap();
        assert!(response.ok);
        let effects = response.side_effects.unwrap();
        assert!(!effects.is_empty());
        // Check env_var side effect
        match &effects[0] {
            PluginSideEffect::EnvVar { key, value, .. } => {
                assert_eq!(key, "TEST_PLUGIN_KEY");
                assert_eq!(value, "test_value");
            }
            _ => panic!("Expected EnvVar side effect"),
        }
    }

    #[tokio::test]
    async fn test_invoke_real_plugin_on_request() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let plugin_path = manifest_dir.join("tests/fixtures/test-plugin.py");
        if !plugin_path.exists() {
            return;
        }

        let request = PluginRequest::OnRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            event: serde_json::json!({"httpMethod": "GET"}),
            function_name: "handler".to_string(),
            config: HashMap::new(),
        };
        let response = invoke_plugin(&plugin_path, &request, 10).await.unwrap();
        assert!(response.ok);
        let event = response.event.unwrap();
        assert_eq!(event["x-plugin-injected"], "true");
    }

    #[tokio::test]
    async fn test_plugin_manager_load_real_plugin() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let plugin_path = manifest_dir.join("tests/fixtures/test-plugin.py");
        if !plugin_path.exists() {
            return;
        }

        let entries = vec![PluginEntry {
            name: "test-plugin".to_string(),
            path: plugin_path.to_str().unwrap().to_string(),
            config: HashMap::new(),
        }];

        let pm = PluginManager::load_plugins(&entries, &manifest_dir)
            .await
            .unwrap();
        assert!(pm.has_plugins());
        assert_eq!(pm.plugin_count(), 1);
        assert_eq!(pm.plugin_names(), vec!["test-plugin"]);
        assert_eq!(pm.plugins_for_resource("aws_s3_bucket").len(), 1);
        assert_eq!(pm.plugins_for_resource("aws_dynamodb_table").len(), 0);
        assert_eq!(pm.request_interceptors().len(), 1);
        assert_eq!(pm.response_interceptors().len(), 1);
    }
}
