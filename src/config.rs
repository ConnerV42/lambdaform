//! Configuration types for Lambdaform
//!
//! Represents parsed Lambda and API Gateway configurations from Terraform.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Lambda function architecture
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    #[default]
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "arm64")]
    Arm64,
}

impl std::fmt::Display for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Architecture::X86_64 => write!(f, "x86_64"),
            Architecture::Arm64 => write!(f, "arm64"),
        }
    }
}

impl std::str::FromStr for Architecture {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "arm64" | "arm" | "graviton" => Architecture::Arm64,
            _ => Architecture::X86_64,
        })
    }
}

impl Architecture {
    pub fn docker_platform(&self) -> &'static str {
        match self {
            Architecture::X86_64 => "linux/amd64",
            Architecture::Arm64 => "linux/arm64",
        }
    }
}

/// Top-level configuration parsed from Terraform
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LambdaformConfig {
    /// Lambda function configurations
    pub functions: Vec<LambdaConfig>,

    /// API Gateway configurations
    pub gateways: Vec<ApiGatewayConfig>,

    /// Lambda layer configurations
    #[serde(default)]
    pub layers: Vec<LayerConfig>,

    /// Step Functions state machine configurations
    #[serde(default)]
    pub state_machines: Vec<StepFunctionConfig>,

    /// DynamoDB table configurations (for integration hints)
    #[serde(default)]
    pub dynamodb_tables: Vec<DynamoDbTableConfig>,

    /// SQS queue configurations
    #[serde(default)]
    pub sqs_queues: Vec<SqsQueueConfig>,

    /// SNS topic configurations
    #[serde(default)]
    pub sns_topics: Vec<SnsTopicConfig>,

    /// Event source mappings (SQS/SNS/DynamoDB → Lambda)
    #[serde(default)]
    pub event_source_mappings: Vec<EventSourceMappingConfig>,

    /// Archive file data sources (for source_path resolution)
    #[serde(default)]
    pub archive_files: Vec<ArchiveFileConfig>,

    /// Lambda Function URL configurations
    #[serde(default)]
    pub function_urls: Vec<FunctionUrlConfig>,

    /// CORS config auto-detected from MOCK integrations in Terraform
    /// Used as fallback when no explicit CORS config in lambdaform.yaml
    #[serde(skip)]
    pub detected_cors: Option<crate::project_config::CorsConfig>,
}

/// Parsed data.archive_file block — used to resolve zip-based source paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveFileConfig {
    /// Resource name (data.archive_file.NAME)
    pub resource_name: String,

    /// source_dir attribute (directory to zip)
    pub source_dir: Option<PathBuf>,

    /// source_file attribute (single file to zip)
    pub source_file: Option<PathBuf>,

    /// output_path attribute (the zip file path)
    pub output_path: Option<PathBuf>,
}

/// DynamoDB table configuration (parsed for integration hints)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDbTableConfig {
    /// Resource name in Terraform (aws_dynamodb_table.NAME)
    pub resource_name: String,

    /// Table name
    pub name: String,

    /// Hash key (partition key)
    pub hash_key: Option<String>,

    /// Range key (sort key)
    pub range_key: Option<String>,

    /// Billing mode (PAY_PER_REQUEST or PROVISIONED)
    #[serde(default = "default_billing_mode")]
    pub billing_mode: String,

    /// Global Secondary Index names
    #[serde(default)]
    pub gsi_names: Vec<String>,

    /// Local Secondary Index names
    #[serde(default)]
    pub lsi_names: Vec<String>,

    /// Whether stream is enabled
    #[serde(default)]
    pub stream_enabled: bool,
}

fn default_billing_mode() -> String {
    "PROVISIONED".to_string()
}

/// SQS queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqsQueueConfig {
    /// Resource name in Terraform (aws_sqs_queue.NAME)
    pub resource_name: String,

    /// Queue name
    pub name: String,

    /// Whether it's a FIFO queue
    #[serde(default)]
    pub fifo_queue: bool,

    /// Visibility timeout seconds
    #[serde(default = "default_visibility_timeout")]
    pub visibility_timeout: u32,
}

fn default_visibility_timeout() -> u32 {
    30
}

/// SNS topic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnsTopicConfig {
    /// Resource name in Terraform (aws_sns_topic.NAME)
    pub resource_name: String,

    /// Topic name
    pub name: String,

    /// Whether it's a FIFO topic
    #[serde(default)]
    pub fifo_topic: bool,
}

/// Event source mapping (connects SQS/SNS/DynamoDB streams to Lambda)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSourceMappingConfig {
    /// Resource name in Terraform
    pub resource_name: String,

    /// Source type
    pub source_type: EventSourceType,

    /// Source resource name (e.g., the SQS queue or DynamoDB table resource name)
    pub source_resource: String,

    /// Target Lambda function resource name
    pub function_resource: String,

    /// Batch size
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,

    /// Whether enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_batch_size() -> u32 {
    10
}
fn default_true() -> bool {
    true
}

/// Event source types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventSourceType {
    Sqs,
    Sns,
    DynamoDb,
    Kinesis,
}

/// Step Functions state machine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFunctionConfig {
    /// Resource name in Terraform (aws_sfn_state_machine.NAME)
    pub resource_name: String,

    /// State machine name
    pub name: String,

    /// State machine type (STANDARD or EXPRESS)
    #[serde(default = "default_sfn_type")]
    pub machine_type: String,

    /// ASL definition (Amazon States Language JSON)
    pub definition: String,

    /// IAM role reference
    pub role_arn_ref: Option<String>,
}

fn default_sfn_type() -> String {
    "STANDARD".to_string()
}

/// Lambda function configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaConfig {
    /// Resource name in Terraform (aws_lambda_function.NAME)
    pub resource_name: String,

    /// Function name (function_name attribute)
    pub function_name: String,

    /// Handler (e.g., "index.handler")
    pub handler: String,

    /// Runtime (e.g., "nodejs20.x", "python3.12")
    pub runtime: Runtime,

    /// Path to source code
    pub source_path: Option<PathBuf>,

    /// Unresolved filename traversal reference (e.g., "data.archive_file.X.output_path")
    /// Used for post-processing resolution against archive_file data sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename_ref: Option<String>,

    /// Environment variables
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u32,

    /// Memory size in MB
    #[serde(default = "default_memory")]
    pub memory_size: u32,

    /// Lambda layer references (resource names of aws_lambda_layer_version)
    #[serde(default)]
    pub layers: Vec<String>,

    /// Architecture (x86_64 or arm64)
    #[serde(default)]
    pub architecture: Architecture,
}

/// Lambda layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    /// Resource name in Terraform (aws_lambda_layer_version.NAME)
    pub resource_name: String,

    /// Layer name
    pub layer_name: String,

    /// Path to layer content directory
    pub source_path: Option<PathBuf>,

    /// Compatible runtimes
    #[serde(default)]
    pub compatible_runtimes: Vec<String>,
}

impl LambdaConfig {
    /// Resolve the effective source directory for this function.
    ///
    /// Priority:
    /// 1. `source_path` if it's an existing directory
    /// 2. If `source_path` ends in `.zip`, check archive_file data sources for source_dir
    /// 3. If `source_path` ends in `.zip`, try the directory without the extension
    /// 4. If `source_path` is a file, use its parent directory
    /// 5. Scan for common source directories (src/, lambda/, functions/, lib/)
    /// 6. Fall back to the project source directory with a warning
    pub fn resolve_source_dir(&self, project_dir: &std::path::Path) -> PathBuf {
        self.resolve_source_dir_with_archives(project_dir, &[])
    }

    /// Resolve source directory with archive_file data sources for zip resolution.
    pub fn resolve_source_dir_with_archives(
        &self,
        project_dir: &std::path::Path,
        archive_files: &[ArchiveFileConfig],
    ) -> PathBuf {
        if let Some(ref sp) = self.source_path {
            // Resolve relative to project dir
            let resolved = if sp.is_relative() {
                project_dir.join(sp)
            } else {
                sp.clone()
            };

            if resolved.is_dir() {
                return resolved;
            }

            // If it's a .zip, try multiple strategies
            if resolved.extension().and_then(|e| e.to_str()) == Some("zip") {
                // Strategy 1: Check archive_file data sources
                if let Some(dir) = self.resolve_from_archive_files(sp, archive_files, project_dir) {
                    return dir;
                }

                // Strategy 2: Try directory with same name as zip (e.g., deploy.zip → deploy/)
                let dir_path = resolved.with_extension("");
                if dir_path.is_dir() {
                    tracing::debug!(
                        "Function '{}': source_path '{}' is a zip, using directory '{}'",
                        self.function_name,
                        resolved.display(),
                        dir_path.display()
                    );
                    return dir_path;
                }

                // Strategy 3: Try parent directory (zip might be in a subdirectory)
                if let Some(parent) = resolved.parent() {
                    if parent.is_dir() && parent != project_dir {
                        tracing::debug!(
                            "Function '{}': using parent of zip path '{}'",
                            self.function_name,
                            parent.display()
                        );
                        return parent.to_path_buf();
                    }
                }

                // Strategy 4: Scan for handler file in common source directories
                if let Some(dir) = self.find_handler_in_common_dirs(project_dir) {
                    tracing::info!(
                        "Function '{}': found handler in '{}' (source_path '{}' is a zip)",
                        self.function_name,
                        dir.display(),
                        resolved.display()
                    );
                    return dir;
                }

                tracing::warn!(
                    "Function '{}': source_path '{}' is a zip archive. \
                     Lambdaform needs source code, not a zip. \
                     Add this to your lambdaform.yaml to fix:\n\n  \
                     functions:\n    {}:\n      source_path: ./path/to/source\n\n  \
                     Or set source_path in Terraform instead of filename pointing to a zip.",
                    self.function_name,
                    resolved.display(),
                    self.resource_name,
                );
            } else if resolved.is_file() {
                // If it's a file, use parent directory
                if let Some(parent) = resolved.parent() {
                    return parent.to_path_buf();
                }
            }
        }

        // source_path is None or didn't resolve — try scanning common dirs for handler
        if let Some(dir) = self.find_handler_in_common_dirs(project_dir) {
            tracing::info!(
                "Function '{}': no source_path set, found handler in '{}'",
                self.function_name,
                dir.display()
            );
            return dir;
        }

        // Final fallback: warn with actionable guidance
        if self.source_path.is_none() {
            tracing::warn!(
                "Function '{}': no source_path or filename attribute found in Terraform. \
                 Lambdaform will look for handler '{}' in the project root. \
                 If your source is elsewhere, add this to lambdaform.yaml:\n\n  \
                 functions:\n    {}:\n      source_path: ./path/to/source\n",
                self.function_name,
                self.handler_filename(),
                self.resource_name,
            );
        }

        project_dir.to_path_buf()
    }

    /// Try to resolve source_path from archive_file data sources.
    /// Matches when an archive_file's output_path matches the function's filename.
    fn resolve_from_archive_files(
        &self,
        source_path: &std::path::Path,
        archive_files: &[ArchiveFileConfig],
        project_dir: &std::path::Path,
    ) -> Option<PathBuf> {
        for archive in archive_files {
            // Match by output_path
            let matches = archive.output_path.as_ref().is_some_and(|op| {
                // Compare normalized paths
                let op_normalized = if op.is_relative() {
                    project_dir.join(op)
                } else {
                    op.clone()
                };
                let sp_normalized = if source_path.is_relative() {
                    project_dir.join(source_path)
                } else {
                    source_path.to_path_buf()
                };
                op_normalized == sp_normalized || op.file_name() == source_path.file_name()
            });

            if matches {
                // Prefer source_dir
                if let Some(ref sd) = archive.source_dir {
                    let resolved = if sd.is_relative() {
                        project_dir.join(sd)
                    } else {
                        sd.clone()
                    };
                    if resolved.is_dir() {
                        tracing::info!(
                            "Function '{}': resolved zip '{}' to source_dir '{}' via data.archive_file.{}",
                            self.function_name,
                            source_path.display(),
                            resolved.display(),
                            archive.resource_name,
                        );
                        return Some(resolved);
                    }
                }

                // Fall back to source_file's parent
                if let Some(ref sf) = archive.source_file {
                    let resolved = if sf.is_relative() {
                        project_dir.join(sf)
                    } else {
                        sf.clone()
                    };
                    if let Some(parent) = resolved.parent() {
                        if parent.is_dir() {
                            tracing::info!(
                                "Function '{}': resolved zip '{}' to source_file parent '{}' via data.archive_file.{}",
                                self.function_name,
                                source_path.display(),
                                parent.display(),
                                archive.resource_name,
                            );
                            return Some(parent.to_path_buf());
                        }
                    }
                }
            }
        }
        None
    }

    /// Scan common source directories for the handler file.
    fn find_handler_in_common_dirs(&self, project_dir: &std::path::Path) -> Option<PathBuf> {
        let handler_file = self.handler_filename();
        let common_dirs = ["src", "lambda", "functions", "lib", "app", "handler"];

        for dir_name in &common_dirs {
            let candidate = project_dir.join(dir_name);
            if candidate.is_dir() && candidate.join(&handler_file).is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Get the expected handler filename based on runtime and handler string.
    fn handler_filename(&self) -> String {
        let module = self.handler.split('.').next().unwrap_or("index");
        match self.runtime {
            Runtime::Nodejs18 | Runtime::Nodejs20 | Runtime::Nodejs22 => {
                format!("{}.js", module)
            }
            Runtime::Python310 | Runtime::Python311 | Runtime::Python312 | Runtime::Python313 => {
                format!("{}.py", module)
            }
            Runtime::Go1 | Runtime::ProvidedAl2 | Runtime::ProvidedAl2023 => {
                "bootstrap".to_string()
            }
            _ => format!("{}.js", module), // default assumption
        }
    }
}

fn default_timeout() -> u32 {
    3
}
fn default_memory() -> u32 {
    128
}

/// Supported Lambda runtimes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    #[serde(rename = "nodejs18.x")]
    Nodejs18,
    #[serde(rename = "nodejs20.x")]
    Nodejs20,
    #[serde(rename = "nodejs22.x")]
    Nodejs22,
    #[serde(rename = "python3.10")]
    Python310,
    #[serde(rename = "python3.11")]
    Python311,
    #[serde(rename = "python3.12")]
    Python312,
    #[serde(rename = "python3.13")]
    Python313,
    #[serde(rename = "go1.x")]
    Go1,
    #[serde(rename = "provided.al2")]
    ProvidedAl2,
    #[serde(rename = "provided.al2023")]
    ProvidedAl2023,
    #[serde(rename = "java8.al2")]
    Java8Al2,
    #[serde(rename = "java11")]
    Java11,
    #[serde(rename = "java17")]
    Java17,
    #[serde(rename = "java21")]
    Java21,
    /// Unknown runtime (will attempt to run anyway)
    Unknown(String),
}

impl Runtime {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "nodejs18.x" => Runtime::Nodejs18,
            "nodejs20.x" => Runtime::Nodejs20,
            "nodejs22.x" => Runtime::Nodejs22,
            "python3.10" => Runtime::Python310,
            "python3.11" => Runtime::Python311,
            "python3.12" => Runtime::Python312,
            "python3.13" => Runtime::Python313,
            "go1.x" => Runtime::Go1,
            "provided.al2" => Runtime::ProvidedAl2,
            "provided.al2023" => Runtime::ProvidedAl2023,
            "java8.al2" => Runtime::Java8Al2,
            "java11" => Runtime::Java11,
            "java17" => Runtime::Java17,
            "java21" => Runtime::Java21,
            other => Runtime::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Runtime::Nodejs18 => "nodejs18.x",
            Runtime::Nodejs20 => "nodejs20.x",
            Runtime::Nodejs22 => "nodejs22.x",
            Runtime::Python310 => "python3.10",
            Runtime::Python311 => "python3.11",
            Runtime::Python312 => "python3.12",
            Runtime::Python313 => "python3.13",
            Runtime::Go1 => "go1.x",
            Runtime::ProvidedAl2 => "provided.al2",
            Runtime::ProvidedAl2023 => "provided.al2023",
            Runtime::Java8Al2 => "java8.al2",
            Runtime::Java11 => "java11",
            Runtime::Java17 => "java17",
            Runtime::Java21 => "java21",
            Runtime::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_nodejs(&self) -> bool {
        matches!(
            self,
            Runtime::Nodejs18 | Runtime::Nodejs20 | Runtime::Nodejs22
        )
    }

    pub fn is_python(&self) -> bool {
        matches!(
            self,
            Runtime::Python310 | Runtime::Python311 | Runtime::Python312 | Runtime::Python313
        )
    }

    pub fn is_java(&self) -> bool {
        matches!(
            self,
            Runtime::Java8Al2 | Runtime::Java11 | Runtime::Java17 | Runtime::Java21
        )
    }

    /// Returns the Docker image tag for Java runtimes
    pub fn java_docker_tag(&self) -> Option<&str> {
        match self {
            Runtime::Java8Al2 => Some("8.al2"),
            Runtime::Java11 => Some("11"),
            Runtime::Java17 => Some("17"),
            Runtime::Java21 => Some("21"),
            _ => None,
        }
    }

    /// Returns true for custom/provided runtimes (Go, Rust, or any compiled binary)
    #[allow(dead_code)]
    pub fn is_custom_runtime(&self) -> bool {
        matches!(
            self,
            Runtime::Go1 | Runtime::ProvidedAl2 | Runtime::ProvidedAl2023
        )
    }
}

/// Lambda Function URL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionUrlConfig {
    /// Terraform resource name (aws_lambda_function_url.NAME)
    pub resource_name: String,

    /// Target Lambda function resource name
    pub function_resource: String,

    /// Authorization type (NONE or AWS_IAM)
    pub auth_type: FunctionUrlAuthType,

    /// CORS configuration (optional)
    pub cors: Option<FunctionUrlCors>,
}

/// Function URL authorization type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FunctionUrlAuthType {
    None,
    AwsIam,
}

/// Function URL CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionUrlCors {
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
    pub expose_headers: Vec<String>,
    pub max_age: Option<u64>,
    pub allow_credentials: bool,
}

/// API Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGatewayConfig {
    /// Resource name in Terraform
    pub resource_name: String,

    /// API name
    pub name: String,

    /// API type (REST or HTTP)
    pub api_type: ApiType,

    /// Routes
    pub routes: Vec<RouteConfig>,

    /// Route selection expression for WebSocket APIs (e.g., "$request.body.action")
    #[serde(default)]
    pub route_selection_expression: Option<String>,
}

/// API Gateway type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApiType {
    Rest,      // v1
    Http,      // v2
    WebSocket, // v2 WebSocket
}

/// Route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// HTTP method
    pub method: HttpMethod,

    /// Path pattern (e.g., "/users/{id}")
    pub path: String,

    /// Target Lambda function resource name
    pub function_resource: String,

    /// Optional authorizer
    pub authorizer: Option<AuthorizerConfig>,
}

/// HTTP methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
    Head,
    Any,
}

impl RouteConfig {
    pub fn method_str(&self) -> &str {
        match self.method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Head => "HEAD",
            HttpMethod::Any => "ANY",
        }
    }
}

/// Authorizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizerConfig {
    /// Authorizer type
    pub auth_type: AuthorizerType,

    /// Lambda function resource (for Lambda authorizers)
    pub function_resource: Option<String>,
}

/// Authorizer types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthorizerType {
    /// Lambda authorizer (token or request)
    Lambda,
    /// Cognito User Pools
    Cognito,
    /// IAM
    Iam,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_from_str_known() {
        assert_eq!(Runtime::from_str("nodejs18.x"), Runtime::Nodejs18);
        assert_eq!(Runtime::from_str("nodejs20.x"), Runtime::Nodejs20);
        assert_eq!(Runtime::from_str("nodejs22.x"), Runtime::Nodejs22);
        assert_eq!(Runtime::from_str("python3.10"), Runtime::Python310);
        assert_eq!(Runtime::from_str("python3.11"), Runtime::Python311);
        assert_eq!(Runtime::from_str("python3.12"), Runtime::Python312);
        assert_eq!(Runtime::from_str("python3.13"), Runtime::Python313);
        assert_eq!(Runtime::from_str("go1.x"), Runtime::Go1);
        assert_eq!(Runtime::from_str("provided.al2"), Runtime::ProvidedAl2);
        assert_eq!(
            Runtime::from_str("provided.al2023"),
            Runtime::ProvidedAl2023
        );
    }

    #[test]
    fn test_runtime_from_str_unknown() {
        assert_eq!(
            Runtime::from_str("ruby3.2"),
            Runtime::Unknown("ruby3.2".to_string())
        );
        assert_eq!(Runtime::from_str(""), Runtime::Unknown("".to_string()));
    }

    #[test]
    fn test_runtime_is_nodejs() {
        assert!(Runtime::Nodejs18.is_nodejs());
        assert!(Runtime::Nodejs20.is_nodejs());
        assert!(Runtime::Nodejs22.is_nodejs());
        assert!(!Runtime::Python312.is_nodejs());
        assert!(!Runtime::Go1.is_nodejs());
        assert!(!Runtime::Unknown("foo".to_string()).is_nodejs());
    }

    #[test]
    fn test_runtime_is_python() {
        assert!(Runtime::Python310.is_python());
        assert!(Runtime::Python311.is_python());
        assert!(Runtime::Python312.is_python());
        assert!(Runtime::Python313.is_python());
        assert!(!Runtime::Nodejs20.is_python());
        assert!(!Runtime::Go1.is_python());
    }

    #[test]
    fn test_runtime_is_custom_runtime() {
        assert!(Runtime::Go1.is_custom_runtime());
        assert!(Runtime::ProvidedAl2.is_custom_runtime());
        assert!(Runtime::ProvidedAl2023.is_custom_runtime());
        assert!(!Runtime::Nodejs20.is_custom_runtime());
        assert!(!Runtime::Python312.is_custom_runtime());
    }

    #[test]
    fn test_route_config_method_str() {
        let cases = vec![
            (HttpMethod::Get, "GET"),
            (HttpMethod::Post, "POST"),
            (HttpMethod::Put, "PUT"),
            (HttpMethod::Patch, "PATCH"),
            (HttpMethod::Delete, "DELETE"),
            (HttpMethod::Options, "OPTIONS"),
            (HttpMethod::Head, "HEAD"),
            (HttpMethod::Any, "ANY"),
        ];
        for (method, expected) in cases {
            let route = RouteConfig {
                method,
                path: "/test".to_string(),
                function_resource: "fn".to_string(),
                authorizer: None,
            };
            assert_eq!(route.method_str(), expected);
        }
    }

    #[test]
    fn test_lambdaform_config_default() {
        let config = LambdaformConfig::default();
        assert!(config.functions.is_empty());
        assert!(config.gateways.is_empty());
    }

    #[test]
    fn test_lambda_config_defaults() {
        assert_eq!(default_timeout(), 3);
        assert_eq!(default_memory(), 128);
    }

    #[test]
    fn test_resolve_source_dir_no_source_path_scans_common_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("index.js"), "exports.handler = () => {}").unwrap();

        let config = LambdaConfig {
            resource_name: "my_func".to_string(),
            function_name: "my_func".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: std::collections::HashMap::new(),
            timeout: 3,
            memory_size: 128,
            layers: vec![],
            architecture: crate::config::Architecture::default(),
        };

        let resolved = config.resolve_source_dir(tmp.path());
        assert_eq!(resolved, src_dir);
    }

    #[test]
    fn test_resolve_source_dir_zip_finds_matching_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let deploy_dir = tmp.path().join("deploy");
        std::fs::create_dir_all(&deploy_dir).unwrap();
        std::fs::write(deploy_dir.join("index.js"), "exports.handler = () => {}").unwrap();
        // Create a fake zip file so the path "exists" as a file check won't pass
        // (we just need the .zip extension to trigger the strategy)

        let config = LambdaConfig {
            resource_name: "my_func".to_string(),
            function_name: "my_func".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: Some(std::path::PathBuf::from("deploy.zip")),
            filename_ref: None,
            environment: std::collections::HashMap::new(),
            timeout: 3,
            memory_size: 128,
            layers: vec![],
            architecture: crate::config::Architecture::default(),
        };

        let resolved = config.resolve_source_dir(tmp.path());
        assert_eq!(resolved, deploy_dir);
    }
}
