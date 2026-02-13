//! HCL Parser for Terraform files
//!
//! Extracts Lambda and API Gateway configurations from .tf files.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::config::*;

/// Parse all Terraform files in a directory
pub fn parse_terraform_dir(dir: &Path) -> Result<LambdaformConfig> {
    let mut config = LambdaformConfig::default();
    
    // Find all .tf files
    for entry in WalkDir::new(dir)
        .max_depth(2) // Don't recurse too deep
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "tf") {
            tracing::debug!("Parsing: {}", path.display());
            parse_tf_file(path, &mut config)?;
        }
    }
    
    Ok(config)
}

/// Intermediate structs for collecting API Gateway resources before resolving
#[derive(Debug, Clone)]
struct ApigwResource {
    resource_name: String,
    rest_api_ref: String,
    parent_ref: String,
    path_part: String,
}

#[derive(Debug, Clone)]
struct ApigwMethod {
    resource_name: String,
    rest_api_ref: String,
    resource_ref: String,
    http_method: String,
}

#[derive(Debug, Clone)]
struct ApigwIntegration {
    rest_api_ref: String,
    resource_ref: String,
    http_method_ref: String,
    lambda_uri_ref: Option<String>,
}

/// Parse a single .tf file
fn parse_tf_file(path: &Path, config: &mut LambdaformConfig) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    
    // Parse HCL
    let body: hcl::Body = hcl::from_str(&content)
        .with_context(|| format!("Failed to parse HCL in {}", path.display()))?;
    
    let mut apigw_resources: Vec<ApigwResource> = Vec::new();
    let mut apigw_methods: Vec<ApigwMethod> = Vec::new();
    let mut apigw_integrations: Vec<ApigwIntegration> = Vec::new();
    
    // Extract resource blocks
    for block in body.blocks() {
        let identifier = block.identifier.to_string();
        if identifier == "resource" {
            let labels: Vec<String> = block.labels.iter().map(|l| label_to_string(l)).collect();
            if labels.len() >= 2 {
                let resource_type = &labels[0];
                let resource_name = &labels[1];
                
                match resource_type.as_str() {
                    "aws_lambda_function" => {
                        if let Some(lambda) = parse_lambda_function(&resource_name, block)? {
                            config.functions.push(lambda);
                        }
                    }
                    "aws_api_gateway_rest_api" => {
                        if let Some(api) = parse_api_gateway_rest(&resource_name, block)? {
                            config.gateways.push(api);
                        }
                    }
                    "aws_api_gateway_resource" => {
                        if let Some(r) = parse_apigw_resource(resource_name, block) {
                            apigw_resources.push(r);
                        }
                    }
                    "aws_api_gateway_method" => {
                        if let Some(m) = parse_apigw_method(resource_name, block) {
                            apigw_methods.push(m);
                        }
                    }
                    "aws_api_gateway_integration" => {
                        if let Some(i) = parse_apigw_integration(block) {
                            apigw_integrations.push(i);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    // Resolve API Gateway routes from resource→method→integration chain
    resolve_api_gateway_routes(config, &apigw_resources, &apigw_methods, &apigw_integrations);
    
    Ok(())
}

/// Parse aws_api_gateway_resource
fn parse_apigw_resource(name: &str, block: &hcl::Block) -> Option<ApigwResource> {
    let body = &block.body;
    Some(ApigwResource {
        resource_name: name.to_string(),
        rest_api_ref: get_traversal_attr(body, "rest_api_id").unwrap_or_default(),
        parent_ref: get_traversal_attr(body, "parent_id").unwrap_or_default(),
        path_part: get_string_attr(body, "path_part")?,
    })
}

/// Parse aws_api_gateway_method
fn parse_apigw_method(name: &str, block: &hcl::Block) -> Option<ApigwMethod> {
    let body = &block.body;
    Some(ApigwMethod {
        resource_name: name.to_string(),
        rest_api_ref: get_traversal_attr(body, "rest_api_id").unwrap_or_default(),
        resource_ref: get_traversal_attr(body, "resource_id").unwrap_or_default(),
        http_method: get_string_attr(body, "http_method")?,
    })
}

/// Parse aws_api_gateway_integration
fn parse_apigw_integration(block: &hcl::Block) -> Option<ApigwIntegration> {
    let body = &block.body;
    Some(ApigwIntegration {
        rest_api_ref: get_traversal_attr(body, "rest_api_id").unwrap_or_default(),
        resource_ref: get_traversal_attr(body, "resource_id").unwrap_or_default(),
        http_method_ref: get_traversal_attr(body, "http_method").or_else(|| get_string_attr(body, "http_method")).unwrap_or_default(),
        lambda_uri_ref: get_traversal_attr(body, "uri"),
    })
}

/// Resolve the API Gateway resource chain into routes
fn resolve_api_gateway_routes(
    config: &mut LambdaformConfig,
    resources: &[ApigwResource],
    methods: &[ApigwMethod],
    integrations: &[ApigwIntegration],
) {
    // Build resource name → path_part lookup
    let resource_paths: HashMap<String, String> = resources
        .iter()
        .map(|r| (r.resource_name.clone(), r.path_part.clone()))
        .collect();
    
    // For each integration, find the matching method and resource to build a route
    for integration in integrations {
        // Extract lambda function resource name from uri ref
        // e.g., "aws_lambda_function.hello.invoke_arn" → "hello"
        let lambda_resource = match &integration.lambda_uri_ref {
            Some(uri) => extract_lambda_name_from_ref(uri),
            None => continue,
        };
        
        // Find the matching method by resource_ref
        // resource_ref is like "aws_api_gateway_resource.hello.id" → "hello"
        let apigw_resource_name = extract_resource_name_from_ref(&integration.resource_ref);
        
        let path_part = match resource_paths.get(&apigw_resource_name) {
            Some(p) => p,
            None => continue,
        };
        
        // Find the method for this resource
        let method = methods.iter().find(|m| {
            extract_resource_name_from_ref(&m.resource_ref) == apigw_resource_name
        });
        
        let http_method = match method {
            Some(m) => parse_http_method(&m.http_method),
            None => HttpMethod::Any,
        };
        
        // Build the path (for now, simple single-level: /{path_part})
        // TODO: Support nested resources by walking parent chain
        let path = format!("/{}", path_part);
        
        let route = RouteConfig {
            method: http_method,
            path,
            function_resource: lambda_resource,
            authorizer: None,
        };
        
        // Find the gateway and add the route
        // For simplicity, add to first gateway (most common case)
        if let Some(gateway) = config.gateways.first_mut() {
            tracing::info!("Resolved route: {} {} → {}", 
                route.method_str(), route.path, route.function_resource);
            gateway.routes.push(route);
        }
    }
}

/// Extract resource name from a Terraform reference like "aws_lambda_function.hello.invoke_arn"
fn extract_lambda_name_from_ref(ref_str: &str) -> String {
    let parts: Vec<&str> = ref_str.split('.').collect();
    if parts.len() >= 2 && parts[0] == "aws_lambda_function" {
        parts[1].to_string()
    } else {
        ref_str.to_string()
    }
}

/// Extract resource name from ref like "aws_api_gateway_resource.hello.id"
fn extract_resource_name_from_ref(ref_str: &str) -> String {
    let parts: Vec<&str> = ref_str.split('.').collect();
    if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        ref_str.to_string()
    }
}

fn parse_http_method(s: &str) -> HttpMethod {
    match s.to_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "HEAD" => HttpMethod::Head,
        "ANY" => HttpMethod::Any,
        _ => HttpMethod::Any,
    }
}

/// Parse aws_lambda_function resource
fn parse_lambda_function(name: &str, block: &hcl::Block) -> Result<Option<LambdaConfig>> {
    let body = &block.body;
    
    // Extract required attributes
    let function_name = get_string_attr(body, "function_name")
        .unwrap_or_else(|| name.to_string());
    
    let handler = match get_string_attr(body, "handler") {
        Some(h) => h,
        None => {
            tracing::warn!("Lambda {} missing handler, skipping", name);
            return Ok(None);
        }
    };
    
    let runtime_str = match get_string_attr(body, "runtime") {
        Some(r) => r,
        None => {
            tracing::warn!("Lambda {} missing runtime, skipping", name);
            return Ok(None);
        }
    };
    
    // Extract optional attributes
    let timeout = get_number_attr(body, "timeout").unwrap_or(3);
    let memory_size = get_number_attr(body, "memory_size").unwrap_or(128);
    
    // Extract environment variables
    let environment = extract_environment(body);
    
    // Try to find source path
    let source_path = get_string_attr(body, "filename")
        .or_else(|| get_string_attr(body, "source_code_hash").and_then(|_| {
            // If there's a source_code_hash, try to find related archive
            get_string_attr(body, "filename")
        }))
        .map(std::path::PathBuf::from);
    
    Ok(Some(LambdaConfig {
        resource_name: name.to_string(),
        function_name,
        handler,
        runtime: Runtime::from_str(&runtime_str),
        source_path,
        environment,
        timeout,
        memory_size,
    }))
}

/// Parse aws_api_gateway_rest_api resource
fn parse_api_gateway_rest(name: &str, block: &hcl::Block) -> Result<Option<ApiGatewayConfig>> {
    let body = &block.body;
    
    let api_name = get_string_attr(body, "name")
        .unwrap_or_else(|| name.to_string());
    
    Ok(Some(ApiGatewayConfig {
        resource_name: name.to_string(),
        name: api_name,
        api_type: ApiType::Rest,
        routes: Vec::new(), // Routes are defined in separate resources
    }))
}

/// Convert a BlockLabel to a String
fn label_to_string(label: &hcl::structure::BlockLabel) -> String {
    match label {
        hcl::structure::BlockLabel::Identifier(ident) => ident.to_string(),
        hcl::structure::BlockLabel::String(s) => s.clone(),
    }
}

/// Get a traversal attribute as a dotted string (e.g., "aws_lambda_function.hello.invoke_arn")
fn get_traversal_attr(body: &hcl::Body, name: &str) -> Option<String> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .and_then(|attr| {
            match &attr.expr {
                hcl::Expression::Traversal(traversal) => {
                    let parts: Vec<String> = std::iter::once(traversal.expr.to_string())
                        .chain(traversal.operators.iter().map(|op| {
                            match op {
                                hcl::TraversalOperator::GetAttr(ident) => ident.to_string(),
                                hcl::TraversalOperator::Index(expr) => format!("{}", expr),
                                _ => String::new(),
                            }
                        }))
                        .collect();
                    Some(parts.join("."))
                }
                hcl::Expression::String(s) => Some(s.to_string()),
                _ => None,
            }
        })
}

/// Get a string attribute from HCL body
fn get_string_attr(body: &hcl::Body, name: &str) -> Option<String> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .and_then(|attr| {
            match &attr.expr {
                hcl::Expression::String(s) => Some(s.to_string()),
                _ => None,
            }
        })
}

/// Get a number attribute from HCL body
fn get_number_attr(body: &hcl::Body, name: &str) -> Option<u32> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .and_then(|attr| {
            match &attr.expr {
                hcl::Expression::Number(n) => n.as_u64().map(|v| v as u32),
                _ => None,
            }
        })
}

/// Extract environment variables from Lambda resource
fn extract_environment(body: &hcl::Body) -> HashMap<String, String> {
    let mut env = HashMap::new();
    
    // Look for environment block
    for block in body.blocks() {
        let identifier = block.identifier.to_string();
        if identifier == "environment" {
            // Look for variables attribute inside
            for attr in block.body.attributes() {
                if attr.key.to_string() == "variables" {
                    if let hcl::Expression::Object(obj) = &attr.expr {
                        for (key, value) in obj.iter() {
                            if let (hcl::ObjectKey::Identifier(k), hcl::Expression::String(v)) = (key, value) {
                                env.insert(k.to_string(), v.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_parse_simple_lambda() {
        let tf_content = r#"
resource "aws_lambda_function" "api_handler" {
  function_name = "my-api-handler"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = 30
  memory_size   = 256
  
  environment {
    variables = {
      TABLE_NAME = "my-table"
      REGION     = "us-west-2"
    }
  }
}
"#;
        
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.tf");
        fs::write(&file_path, tf_content).unwrap();
        
        let config = parse_terraform_dir(dir.path()).unwrap();
        
        assert_eq!(config.functions.len(), 1);
        let lambda = &config.functions[0];
        assert_eq!(lambda.function_name, "my-api-handler");
        assert_eq!(lambda.handler, "index.handler");
        assert_eq!(lambda.runtime, Runtime::Nodejs20);
        assert_eq!(lambda.timeout, 30);
        assert_eq!(lambda.memory_size, 256);
        assert_eq!(lambda.environment.get("TABLE_NAME"), Some(&"my-table".to_string()));
    }
}
