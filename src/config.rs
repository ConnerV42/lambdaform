//! Configuration types for Lambdaform
//!
//! Represents parsed Lambda and API Gateway configurations from Terraform.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
    #[serde(rename = "python3.10")]
    Python310,
    #[serde(rename = "python3.11")]
    Python311,
    #[serde(rename = "python3.12")]
    Python312,
    #[serde(rename = "go1.x")]
    Go1,
    #[serde(rename = "provided.al2")]
    ProvidedAl2,
    #[serde(rename = "provided.al2023")]
    ProvidedAl2023,
    /// Unknown runtime (will attempt to run anyway)
    Unknown(String),
}

impl Runtime {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "nodejs18.x" => Runtime::Nodejs18,
            "nodejs20.x" => Runtime::Nodejs20,
            "python3.10" => Runtime::Python310,
            "python3.11" => Runtime::Python311,
            "python3.12" => Runtime::Python312,
            "go1.x" => Runtime::Go1,
            "provided.al2" => Runtime::ProvidedAl2,
            "provided.al2023" => Runtime::ProvidedAl2023,
            other => Runtime::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Runtime::Nodejs18 => "nodejs18.x",
            Runtime::Nodejs20 => "nodejs20.x",
            Runtime::Python310 => "python3.10",
            Runtime::Python311 => "python3.11",
            Runtime::Python312 => "python3.12",
            Runtime::Go1 => "go1.x",
            Runtime::ProvidedAl2 => "provided.al2",
            Runtime::ProvidedAl2023 => "provided.al2023",
            Runtime::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_nodejs(&self) -> bool {
        matches!(self, Runtime::Nodejs18 | Runtime::Nodejs20)
    }

    pub fn is_python(&self) -> bool {
        matches!(
            self,
            Runtime::Python310 | Runtime::Python311 | Runtime::Python312
        )
    }

    #[allow(dead_code)]
    pub fn is_go(&self) -> bool {
        matches!(
            self,
            Runtime::Go1 | Runtime::ProvidedAl2 | Runtime::ProvidedAl2023
        )
    }
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
        assert_eq!(Runtime::from_str("python3.10"), Runtime::Python310);
        assert_eq!(Runtime::from_str("python3.11"), Runtime::Python311);
        assert_eq!(Runtime::from_str("python3.12"), Runtime::Python312);
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
        assert!(!Runtime::Python312.is_nodejs());
        assert!(!Runtime::Go1.is_nodejs());
        assert!(!Runtime::Unknown("foo".to_string()).is_nodejs());
    }

    #[test]
    fn test_runtime_is_python() {
        assert!(Runtime::Python310.is_python());
        assert!(Runtime::Python311.is_python());
        assert!(Runtime::Python312.is_python());
        assert!(!Runtime::Nodejs20.is_python());
        assert!(!Runtime::Go1.is_python());
    }

    #[test]
    fn test_runtime_is_go() {
        assert!(Runtime::Go1.is_go());
        assert!(Runtime::ProvidedAl2.is_go());
        assert!(Runtime::ProvidedAl2023.is_go());
        assert!(!Runtime::Nodejs20.is_go());
        assert!(!Runtime::Python312.is_go());
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
}
