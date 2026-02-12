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

/// Parse a single .tf file
fn parse_tf_file(path: &Path, config: &mut LambdaformConfig) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    
    // Parse HCL
    let body: hcl::Body = hcl::from_str(&content)
        .with_context(|| format!("Failed to parse HCL in {}", path.display()))?;
    
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
                    // TODO: Parse other resource types
                    _ => {}
                }
            }
        }
    }
    
    Ok(())
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
