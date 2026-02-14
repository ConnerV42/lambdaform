//! Configuration types for Lambdaform
//!
//! Represents parsed Lambda and API Gateway configurations from Terraform.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level configuration parsed from Terraform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaformConfig {
    /// Lambda function configurations
    pub functions: Vec<LambdaConfig>,
    
    /// API Gateway configurations
    pub gateways: Vec<ApiGatewayConfig>,
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
}

fn default_timeout() -> u32 { 3 }
fn default_memory() -> u32 { 128 }

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
    
    pub fn is_nodejs(&self) -> bool {
        matches!(self, Runtime::Nodejs18 | Runtime::Nodejs20)
    }
    
    pub fn is_python(&self) -> bool {
        matches!(self, Runtime::Python310 | Runtime::Python311 | Runtime::Python312)
    }
    
    pub fn is_go(&self) -> bool {
        matches!(self, Runtime::Go1 | Runtime::ProvidedAl2 | Runtime::ProvidedAl2023)
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
}

/// API Gateway type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApiType {
    Rest,  // v1
    Http,  // v2
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

impl Default for LambdaformConfig {
    fn default() -> Self {
        Self {
            functions: Vec::new(),
            gateways: Vec::new(),
        }
    }
}
