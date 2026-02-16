//! HCL Parser for Terraform files
//!
//! Extracts Lambda and API Gateway configurations from .tf files.
//! Supports Terraform variable resolution from variable blocks and .tfvars files.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::config::*;

/// Resolves Terraform variable references (var.xxx) to their values.
///
/// Resolution order (last wins):
/// 1. `variable` block `default` values from .tf files
/// 2. `terraform.tfvars` (auto-loaded)
/// 3. `*.auto.tfvars` (auto-loaded, alphabetical)
#[derive(Debug, Default, Clone)]
pub struct VariableResolver {
    pub(crate) variables: HashMap<String, String>,
}

impl VariableResolver {
    /// Build a resolver by scanning a directory for variable definitions and .tfvars files.
    /// Extra var_files are loaded last (highest priority), matching `terraform -var-file` behavior.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        Self::from_dir_with_var_files(dir, &[])
    }

    /// Build a resolver with additional -var-file paths.
    pub fn from_dir_with_var_files(dir: &Path, var_files: &[std::path::PathBuf]) -> Result<Self> {
        let mut resolver = Self::default();

        // Pass 1: Collect variable defaults from .tf files
        let mut tf_files: Vec<_> = Vec::new();
        for entry in WalkDir::new(dir)
            .max_depth(2)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path().to_path_buf();
            if path.extension().is_some_and(|ext| ext == "tf") {
                tf_files.push(path);
            }
        }

        for path in &tf_files {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            if let Ok(body) = hcl::from_str::<hcl::Body>(&content) {
                resolver.collect_variable_defaults(&body);
            }
        }

        // Pass 2: Load terraform.tfvars (auto-loaded by Terraform)
        let tfvars_path = dir.join("terraform.tfvars");
        if tfvars_path.exists() {
            resolver.load_tfvars(&tfvars_path)?;
        }

        // Pass 3: Load *.auto.tfvars (alphabetical order)
        let mut auto_tfvars: Vec<_> = Vec::new();
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".auto.tfvars") {
                    auto_tfvars.push(path);
                }
            }
        }
        auto_tfvars.sort();
        for path in &auto_tfvars {
            resolver.load_tfvars(path)?;
        }

        // Pass 4: Load explicit -var-file paths (highest priority)
        for path in var_files {
            resolver.load_tfvars(path)?;
        }

        if !resolver.variables.is_empty() {
            tracing::info!(
                "Resolved {} Terraform variable(s)",
                resolver.variables.len()
            );
            for (k, v) in &resolver.variables {
                tracing::debug!("  var.{} = {:?}", k, v);
            }
        }

        Ok(resolver)
    }

    /// Extract default values from `variable` blocks.
    fn collect_variable_defaults(&mut self, body: &hcl::Body) {
        for block in body.blocks() {
            if block.identifier.to_string() == "variable" {
                if let Some(label) = block.labels.first() {
                    let var_name = label_to_string(label);
                    // Look for a `default` attribute
                    for attr in block.body.attributes() {
                        if attr.key.to_string() == "default" {
                            if let Some(val) = expr_to_string(&attr.expr) {
                                self.variables.insert(var_name.clone(), val);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Load variable values from a .tfvars file.
    fn load_tfvars(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let body: hcl::Body = hcl::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        for attr in body.attributes() {
            let key = attr.key.to_string();
            if let Some(val) = expr_to_string(&attr.expr) {
                self.variables.insert(key, val);
            }
        }

        tracing::debug!("Loaded tfvars from {}", path.display());
        Ok(())
    }

    /// Resolve a string that may contain `var.xxx` references.
    /// Handles both bare traversals (`var.prefix`) and template interpolation (`"${var.prefix}-api"`).
    pub fn resolve(&self, value: &str) -> String {
        if self.variables.is_empty() || !value.contains("var.") {
            return value.to_string();
        }

        // Replace ${var.name} interpolations
        let mut result = value.to_string();
        for (name, val) in &self.variables {
            let pattern = format!("${{var.{}}}", name);
            result = result.replace(&pattern, val);
        }
        result
    }

    /// Resolve a traversal expression like `var.prefix` to its value.
    pub fn resolve_traversal(&self, traversal_str: &str) -> Option<String> {
        if let Some(var_name) = traversal_str.strip_prefix("var.") {
            self.variables.get(var_name).cloned()
        } else {
            None
        }
    }
}

/// Extract a string value from an HCL expression (for variable defaults and tfvars).
fn expr_to_string(expr: &hcl::Expression) -> Option<String> {
    match expr {
        hcl::Expression::String(s) => Some(s.to_string()),
        hcl::Expression::Number(n) => Some(n.to_string()),
        hcl::Expression::Bool(b) => Some(b.to_string()),
        hcl::Expression::TemplateExpr(t) => Some(t.to_string()),
        _ => None,
    }
}

/// Parse all Terraform files in a directory
pub fn parse_terraform_dir(dir: &Path) -> Result<LambdaformConfig> {
    parse_terraform_dir_with_var_files(dir, &[])
}

/// Parse all Terraform files in a directory with additional -var-file paths
pub fn parse_terraform_dir_with_var_files(
    dir: &Path,
    var_files: &[std::path::PathBuf],
) -> Result<LambdaformConfig> {
    parse_terraform_dir_recursive(dir, var_files, 0)
}

/// Recursively parse Terraform files, following local module sources.
/// `depth` prevents infinite recursion from circular module references.
fn parse_terraform_dir_recursive(
    dir: &Path,
    var_files: &[std::path::PathBuf],
    depth: u32,
) -> Result<LambdaformConfig> {
    const MAX_MODULE_DEPTH: u32 = 10;
    if depth > MAX_MODULE_DEPTH {
        tracing::warn!(
            "Module nesting depth exceeded {} at {}, skipping",
            MAX_MODULE_DEPTH,
            dir.display()
        );
        return Ok(LambdaformConfig::default());
    }

    let resolver = VariableResolver::from_dir_with_var_files(dir, var_files)?;

    let mut config = LambdaformConfig::default();

    // Collect .tf files (don't recurse into subdirs — modules handle that)
    let mut tf_files: Vec<std::path::PathBuf> = Vec::new();
    for entry in WalkDir::new(dir)
        .max_depth(1)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "tf") {
            tf_files.push(path.to_path_buf());
        }
    }

    for path in &tf_files {
        tracing::debug!("Parsing: {}", path.display());
        parse_tf_file(path, &mut config, &resolver)?;
    }

    // Scan for module blocks and recursively parse local modules
    let modules = collect_module_blocks(dir, &resolver)?;
    for module in modules {
        // Only follow local source paths (starting with ./ or ../ or no protocol)
        if is_local_module_source(&module.source) {
            let module_dir = dir.join(&module.source);
            let module_dir = match module_dir.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "Module '{}' source '{}' not found: {}",
                        module.name,
                        module.source,
                        e
                    );
                    continue;
                }
            };

            tracing::info!(
                "Parsing module '{}' from {}",
                module.name,
                module_dir.display()
            );

            // Build a resolver for the module with passed-in variable values
            let mut module_config = parse_module_dir(&module_dir, &module.variables, depth + 1)?;

            // Prefix resource names with module name for namespacing
            // This allows referencing module.X.resource_name in the parent
            let prefix = &module.name;
            for func in &mut module_config.functions {
                func.resource_name = format!("{}.{}", prefix, func.resource_name);
            }
            for gw in &mut module_config.gateways {
                gw.resource_name = format!("{}.{}", prefix, gw.resource_name);
            }
            for layer in &mut module_config.layers {
                layer.resource_name = format!("{}.{}", prefix, layer.resource_name);
            }
            for sm in &mut module_config.state_machines {
                sm.resource_name = format!("{}.{}", prefix, sm.resource_name);
            }
            for table in &mut module_config.dynamodb_tables {
                table.resource_name = format!("{}.{}", prefix, table.resource_name);
            }
            for queue in &mut module_config.sqs_queues {
                queue.resource_name = format!("{}.{}", prefix, queue.resource_name);
            }
            for topic in &mut module_config.sns_topics {
                topic.resource_name = format!("{}.{}", prefix, topic.resource_name);
            }
            for esm in &mut module_config.event_source_mappings {
                esm.resource_name = format!("{}.{}", prefix, esm.resource_name);
            }

            // Merge module config into parent
            config.functions.extend(module_config.functions);
            config.gateways.extend(module_config.gateways);
            config.layers.extend(module_config.layers);
            config.state_machines.extend(module_config.state_machines);
            config.dynamodb_tables.extend(module_config.dynamodb_tables);
            config.sqs_queues.extend(module_config.sqs_queues);
            config.sns_topics.extend(module_config.sns_topics);
            config
                .event_source_mappings
                .extend(module_config.event_source_mappings);
        } else {
            tracing::debug!(
                "Skipping remote module '{}' (source: {})",
                module.name,
                module.source
            );
        }
    }

    Ok(config)
}

/// A parsed Terraform module block
#[derive(Debug)]
struct ModuleBlock {
    name: String,
    source: String,
    variables: HashMap<String, String>,
}

/// Check if a module source is local (relative path)
fn is_local_module_source(source: &str) -> bool {
    source.starts_with("./") || source.starts_with("../")
}

/// Parse a module directory with variable overrides from the parent module block
fn parse_module_dir(
    dir: &Path,
    variable_overrides: &HashMap<String, String>,
    depth: u32,
) -> Result<LambdaformConfig> {
    const MAX_MODULE_DEPTH: u32 = 10;
    if depth > MAX_MODULE_DEPTH {
        tracing::warn!(
            "Module nesting depth exceeded {} at {}, skipping",
            MAX_MODULE_DEPTH,
            dir.display()
        );
        return Ok(LambdaformConfig::default());
    }

    // Build a resolver that includes the parent's variable overrides
    let mut resolver = VariableResolver::from_dir(dir)?;
    // Override with values passed from the module block
    for (k, v) in variable_overrides {
        resolver.variables.insert(k.clone(), v.clone());
    }

    let mut config = LambdaformConfig::default();

    // Parse .tf files in the module directory (depth 1 only)
    for entry in WalkDir::new(dir)
        .max_depth(1)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "tf") {
            tracing::debug!("Parsing module file: {}", path.display());
            parse_tf_file(path, &mut config, &resolver)?;
        }
    }

    // Resolve source_path relative to module directory
    for func in &mut config.functions {
        if let Some(ref sp) = func.source_path {
            if sp.is_relative() {
                func.source_path = Some(dir.join(sp));
            }
        }
    }
    for layer in &mut config.layers {
        if let Some(ref sp) = layer.source_path {
            if sp.is_relative() {
                layer.source_path = Some(dir.join(sp));
            }
        }
    }

    // Recursively handle nested modules
    let modules = collect_module_blocks(dir, &resolver)?;
    for module in modules {
        if is_local_module_source(&module.source) {
            let module_dir = dir.join(&module.source);
            let module_dir = match module_dir.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "Nested module '{}' source '{}' not found: {}",
                        module.name,
                        module.source,
                        e
                    );
                    continue;
                }
            };
            let mut nested = parse_module_dir(&module_dir, &module.variables, depth + 1)?;
            let prefix = &module.name;
            for func in &mut nested.functions {
                func.resource_name = format!("{}.{}", prefix, func.resource_name);
            }
            for gw in &mut nested.gateways {
                gw.resource_name = format!("{}.{}", prefix, gw.resource_name);
            }
            for layer in &mut nested.layers {
                layer.resource_name = format!("{}.{}", prefix, layer.resource_name);
            }
            for sm in &mut nested.state_machines {
                sm.resource_name = format!("{}.{}", prefix, sm.resource_name);
            }
            for table in &mut nested.dynamodb_tables {
                table.resource_name = format!("{}.{}", prefix, table.resource_name);
            }
            for queue in &mut nested.sqs_queues {
                queue.resource_name = format!("{}.{}", prefix, queue.resource_name);
            }
            for topic in &mut nested.sns_topics {
                topic.resource_name = format!("{}.{}", prefix, topic.resource_name);
            }
            for esm in &mut nested.event_source_mappings {
                esm.resource_name = format!("{}.{}", prefix, esm.resource_name);
            }
            config.functions.extend(nested.functions);
            config.gateways.extend(nested.gateways);
            config.layers.extend(nested.layers);
            config.state_machines.extend(nested.state_machines);
            config.dynamodb_tables.extend(nested.dynamodb_tables);
            config.sqs_queues.extend(nested.sqs_queues);
            config.sns_topics.extend(nested.sns_topics);
            config
                .event_source_mappings
                .extend(nested.event_source_mappings);
        }
    }

    Ok(config)
}

/// Scan .tf files in a directory for `module` blocks and extract source + variables
fn collect_module_blocks(dir: &Path, resolver: &VariableResolver) -> Result<Vec<ModuleBlock>> {
    let mut modules = Vec::new();

    for entry in WalkDir::new(dir)
        .max_depth(1)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "tf") {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let body: hcl::Body = match hcl::from_str(&content) {
                Ok(b) => b,
                Err(_) => continue,
            };

            for block in body.blocks() {
                if block.identifier.to_string() == "module" {
                    if let Some(label) = block.labels.first() {
                        let name = label_to_string(label);
                        let source = get_string_attr_resolved(&block.body, "source", resolver);

                        if let Some(source) = source {
                            // Collect variable assignments from the module block
                            let mut variables = HashMap::new();
                            for attr in block.body.attributes() {
                                let key = attr.key.to_string();
                                if key == "source"
                                    || key == "providers"
                                    || key == "depends_on"
                                    || key == "count"
                                    || key == "for_each"
                                {
                                    continue; // Skip meta-arguments
                                }
                                if let Some(val) = expr_to_string(&attr.expr) {
                                    variables.insert(key, resolver.resolve(&val));
                                } else if let hcl::Expression::Traversal(traversal) = &attr.expr {
                                    // Handle var.xxx references passed to module
                                    let parts: Vec<String> =
                                        std::iter::once(traversal.expr.to_string())
                                            .chain(traversal.operators.iter().map(|op| match op {
                                                hcl::TraversalOperator::GetAttr(ident) => {
                                                    ident.to_string()
                                                }
                                                _ => String::new(),
                                            }))
                                            .collect();
                                    let trav_str = parts.join(".");
                                    if let Some(resolved) = resolver.resolve_traversal(&trav_str) {
                                        variables.insert(key, resolved);
                                    }
                                }
                            }

                            modules.push(ModuleBlock {
                                name,
                                source,
                                variables,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(modules)
}

/// Intermediate structs for collecting API Gateway resources before resolving
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ApigwResource {
    resource_name: String,
    rest_api_ref: String,
    parent_ref: String,
    path_part: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ApigwMethod {
    resource_name: String,
    rest_api_ref: String,
    resource_ref: String,
    http_method: String,
    authorizer_ref: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ApigwIntegration {
    rest_api_ref: String,
    resource_ref: String,
    http_method_ref: String,
    lambda_uri_ref: Option<String>,
}

/// API Gateway authorizer intermediate structs
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ApigwAuthorizer {
    resource_name: String,
    rest_api_ref: String,
    lambda_uri_ref: Option<String>,
    auth_type: String, // TOKEN, REQUEST, COGNITO_USER_POOLS
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Apigwv2Authorizer {
    resource_name: String,
    api_ref: String,
    lambda_uri_ref: Option<String>,
    auth_type: String, // REQUEST, JWT
}

/// HTTP API Gateway v2 intermediate structs
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Apigwv2Route {
    resource_name: String,
    api_ref: String,
    route_key: String,
    target_ref: Option<String>,
    authorizer_ref: Option<String>,
}

#[derive(Debug, Clone)]
struct Apigwv2Integration {
    resource_name: String,
    api_ref: String,
    lambda_uri_ref: Option<String>,
}

/// Parse a single .tf file
fn parse_tf_file(
    path: &Path,
    config: &mut LambdaformConfig,
    resolver: &VariableResolver,
) -> Result<()> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    // Parse HCL
    let body: hcl::Body = hcl::from_str(&content)
        .with_context(|| format!("Failed to parse HCL in {}", path.display()))?;

    let mut apigw_resources: Vec<ApigwResource> = Vec::new();
    let mut apigw_methods: Vec<ApigwMethod> = Vec::new();
    let mut apigw_integrations: Vec<ApigwIntegration> = Vec::new();
    let mut apigwv2_routes: Vec<Apigwv2Route> = Vec::new();
    let mut apigwv2_integrations: Vec<Apigwv2Integration> = Vec::new();
    let mut apigw_authorizers: Vec<ApigwAuthorizer> = Vec::new();
    let mut apigwv2_authorizers: Vec<Apigwv2Authorizer> = Vec::new();

    // Extract resource blocks
    for block in body.blocks() {
        let identifier = block.identifier.to_string();
        if identifier == "resource" {
            let labels: Vec<String> = block.labels.iter().map(label_to_string).collect();
            if labels.len() >= 2 {
                let resource_type = &labels[0];
                let resource_name = &labels[1];

                match resource_type.as_str() {
                    "aws_lambda_function" => {
                        if let Some(lambda) = parse_lambda_function(resource_name, block, resolver)?
                        {
                            config.functions.push(lambda);
                        }
                    }
                    "aws_api_gateway_rest_api" => {
                        if let Some(api) = parse_api_gateway_rest(resource_name, block, resolver)? {
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
                    "aws_apigatewayv2_api" => {
                        if let Some(api) = parse_apigatewayv2_api(resource_name, block, resolver)? {
                            config.gateways.push(api);
                        }
                    }
                    "aws_apigatewayv2_route" => {
                        if let Some(r) = parse_apigatewayv2_route(resource_name, block) {
                            apigwv2_routes.push(r);
                        }
                    }
                    "aws_apigatewayv2_integration" => {
                        if let Some(i) = parse_apigatewayv2_integration(resource_name, block) {
                            apigwv2_integrations.push(i);
                        }
                    }
                    "aws_api_gateway_authorizer" => {
                        if let Some(a) = parse_apigw_authorizer(resource_name, block) {
                            apigw_authorizers.push(a);
                        }
                    }
                    "aws_apigatewayv2_authorizer" => {
                        if let Some(a) = parse_apigwv2_authorizer(resource_name, block) {
                            apigwv2_authorizers.push(a);
                        }
                    }
                    "aws_lambda_layer_version" => {
                        if let Some(layer) = parse_lambda_layer(resource_name, block)? {
                            config.layers.push(layer);
                        }
                    }
                    "aws_sfn_state_machine" => {
                        if let Some(sm) = parse_sfn_state_machine(resource_name, block)? {
                            config.state_machines.push(sm);
                        }
                    }
                    "aws_dynamodb_table" => {
                        if let Some(table) = parse_dynamodb_table(resource_name, block, resolver)? {
                            config.dynamodb_tables.push(table);
                        }
                    }
                    "aws_sqs_queue" => {
                        if let Some(queue) = parse_sqs_queue(resource_name, block, resolver)? {
                            config.sqs_queues.push(queue);
                        }
                    }
                    "aws_sns_topic" => {
                        if let Some(topic) = parse_sns_topic(resource_name, block, resolver)? {
                            config.sns_topics.push(topic);
                        }
                    }
                    "aws_lambda_event_source_mapping" => {
                        if let Some(esm) = parse_event_source_mapping(resource_name, block)? {
                            config.event_source_mappings.push(esm);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Resolve cross-resource references in Lambda environment variables
    // (e.g., aws_dynamodb_table.meetings.name → resolved table name)
    resolve_cross_resource_env_vars(config);

    // Resolve API Gateway v1 routes from resource→method→integration chain
    resolve_api_gateway_routes(
        config,
        &apigw_resources,
        &apigw_methods,
        &apigw_integrations,
        &apigw_authorizers,
    );

    // Resolve API Gateway v2 routes from route→integration chain
    resolve_apigatewayv2_routes(
        config,
        &apigwv2_routes,
        &apigwv2_integrations,
        &apigwv2_authorizers,
    );

    Ok(())
}

/// Resolve cross-resource references in Lambda environment variables.
/// Builds a lookup table from parsed resources, then replaces unresolved traversal
/// references in Lambda env vars (e.g., `aws_dynamodb_table.meetings.name` → table name,
/// `aws_sqs_queue.ingest_queue.id` → queue name).
fn resolve_cross_resource_env_vars(config: &mut LambdaformConfig) {
    // Build resource attribute lookup: "aws_type.resource_name.attr" → value
    let mut resource_attrs: HashMap<String, String> = HashMap::new();

    for table in &config.dynamodb_tables {
        let prefix = format!("aws_dynamodb_table.{}", table.resource_name);
        resource_attrs.insert(format!("{}.name", prefix), table.name.clone());
        // .arn is typically needed but we generate a synthetic one for local dev
        resource_attrs.insert(
            format!("{}.arn", prefix),
            format!("arn:aws:dynamodb:local:000000000000:table/{}", table.name),
        );
        resource_attrs.insert(format!("{}.id", prefix), table.name.clone());
    }

    for queue in &config.sqs_queues {
        let prefix = format!("aws_sqs_queue.{}", queue.resource_name);
        resource_attrs.insert(format!("{}.id", prefix), queue.name.clone());
        resource_attrs.insert(format!("{}.name", prefix), queue.name.clone());
        resource_attrs.insert(
            format!("{}.arn", prefix),
            format!("arn:aws:sqs:local:000000000000:{}", queue.name),
        );
        resource_attrs.insert(
            format!("{}.url", prefix),
            format!(
                "https://sqs.local.amazonaws.com/000000000000/{}",
                queue.name
            ),
        );
    }

    for topic in &config.sns_topics {
        let prefix = format!("aws_sns_topic.{}", topic.resource_name);
        resource_attrs.insert(
            format!("{}.arn", prefix),
            format!("arn:aws:sns:local:000000000000:{}", topic.name),
        );
        resource_attrs.insert(format!("{}.id", prefix), topic.name.clone());
        resource_attrs.insert(format!("{}.name", prefix), topic.name.clone());
    }

    // Also add IAM role references (commonly referenced)
    // We don't track IAM roles in config, but we can provide a synthetic ARN
    // for now, just skip unknown references gracefully

    if resource_attrs.is_empty() {
        return;
    }

    tracing::debug!(
        "Built cross-resource lookup with {} entries",
        resource_attrs.len()
    );

    // Resolve placeholder env vars in Lambda functions
    // Placeholders look like ${aws_dynamodb_table.meetings.name}
    let mut total_resolved = 0;
    for lambda in &mut config.functions {
        for value in lambda.environment.values_mut() {
            if value.starts_with("${") && value.ends_with('}') {
                let ref_key = &value[2..value.len() - 1];
                if let Some(resolved) = resource_attrs.get(ref_key) {
                    tracing::debug!("Resolved env ref {} → {}", ref_key, resolved);
                    *value = resolved.clone();
                    total_resolved += 1;
                }
            }
        }
    }

    if total_resolved > 0 {
        tracing::info!(
            "Resolved {} cross-resource reference(s) in Lambda environment variables",
            total_resolved
        );
    }
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
        authorizer_ref: get_traversal_attr(body, "authorizer_id"),
    })
}

/// Parse aws_api_gateway_integration
fn parse_apigw_integration(block: &hcl::Block) -> Option<ApigwIntegration> {
    let body = &block.body;
    Some(ApigwIntegration {
        rest_api_ref: get_traversal_attr(body, "rest_api_id").unwrap_or_default(),
        resource_ref: get_traversal_attr(body, "resource_id").unwrap_or_default(),
        http_method_ref: get_traversal_attr(body, "http_method")
            .or_else(|| get_string_attr(body, "http_method"))
            .unwrap_or_default(),
        lambda_uri_ref: get_traversal_attr(body, "uri"),
    })
}

/// Resolve the API Gateway resource chain into routes
fn resolve_api_gateway_routes(
    config: &mut LambdaformConfig,
    resources: &[ApigwResource],
    methods: &[ApigwMethod],
    integrations: &[ApigwIntegration],
    authorizers: &[ApigwAuthorizer],
) {
    // Build resource name → path_part lookup
    let resource_paths: HashMap<String, String> = resources
        .iter()
        .map(|r| (r.resource_name.clone(), r.path_part.clone()))
        .collect();

    // Build resource name → parent resource name lookup for nested path resolution
    let resource_parents: HashMap<String, String> = resources
        .iter()
        .filter_map(|r| {
            let parent_name = extract_resource_name_from_ref(&r.parent_ref);
            // Only include if parent is another resource (not a root_resource_id)
            if resource_paths.contains_key(&parent_name) {
                Some((r.resource_name.clone(), parent_name))
            } else {
                None
            }
        })
        .collect();

    // Build authorizer name → AuthorizerConfig lookup
    let authorizer_map: HashMap<String, AuthorizerConfig> = authorizers
        .iter()
        .map(|a| {
            let auth_type = match a.auth_type.as_str() {
                "TOKEN" | "REQUEST" => AuthorizerType::Lambda,
                "COGNITO_USER_POOLS" => AuthorizerType::Cognito,
                _ => AuthorizerType::Lambda,
            };
            let function_resource = a
                .lambda_uri_ref
                .as_ref()
                .map(|uri| extract_lambda_name_from_ref(uri));
            (
                a.resource_name.clone(),
                AuthorizerConfig {
                    auth_type,
                    function_resource,
                },
            )
        })
        .collect();

    // For each integration, find the matching method and resource to build a route
    for integration in integrations {
        // Extract lambda function resource name from uri ref
        let lambda_resource = match &integration.lambda_uri_ref {
            Some(uri) => extract_lambda_name_from_ref(uri),
            None => continue,
        };

        let apigw_resource_name = extract_resource_name_from_ref(&integration.resource_ref);

        let path_part = match resource_paths.get(&apigw_resource_name) {
            Some(p) => p,
            None => continue,
        };

        // Find the method for this resource
        let method = methods
            .iter()
            .find(|m| extract_resource_name_from_ref(&m.resource_ref) == apigw_resource_name);

        let http_method = match &method {
            Some(m) => parse_http_method(&m.http_method),
            None => HttpMethod::Any,
        };

        // Resolve authorizer from method's authorizer_id ref
        let authorizer = method.and_then(|m| {
            m.authorizer_ref.as_ref().and_then(|ref_str| {
                let auth_name = extract_resource_name_from_ref(ref_str);
                authorizer_map.get(&auth_name).cloned()
            })
        });

        // Walk parent chain to build full nested path (e.g. /api/v1/users/{id})
        let path = {
            let mut parts = vec![path_part.as_str()];
            let mut current = apigw_resource_name.as_str();
            // Walk up parent_ref chain, max 20 levels to prevent infinite loops
            for _ in 0..20 {
                match resource_parents.get(current) {
                    Some(parent_name) => {
                        if let Some(parent_path) = resource_paths.get(parent_name) {
                            parts.push(parent_path.as_str());
                            current = parent_name.as_str();
                        } else {
                            break;
                        }
                    }
                    None => break,
                }
            }
            parts.reverse();
            format!("/{}", parts.join("/"))
        };

        let route = RouteConfig {
            method: http_method,
            path,
            function_resource: lambda_resource,
            authorizer,
        };

        if let Some(gateway) = config.gateways.first_mut() {
            if route.authorizer.is_some() {
                tracing::info!(
                    "Resolved route: {} {} → {} (with authorizer)",
                    route.method_str(),
                    route.path,
                    route.function_resource
                );
            } else {
                tracing::info!(
                    "Resolved route: {} {} → {}",
                    route.method_str(),
                    route.path,
                    route.function_resource
                );
            }
            gateway.routes.push(route);
        }
    }
}

/// Parse aws_apigatewayv2_api resource
fn parse_apigatewayv2_api(
    name: &str,
    block: &hcl::Block,
    resolver: &VariableResolver,
) -> Result<Option<ApiGatewayConfig>> {
    let body = &block.body;
    let api_name =
        get_string_attr_resolved(body, "name", resolver).unwrap_or_else(|| name.to_string());
    let protocol_type =
        get_string_attr(body, "protocol_type").unwrap_or_else(|| "HTTP".to_string());
    let route_selection_expression = get_string_attr(body, "route_selection_expression");

    let api_type = if protocol_type.eq_ignore_ascii_case("WEBSOCKET") {
        ApiType::WebSocket
    } else {
        ApiType::Http
    };

    Ok(Some(ApiGatewayConfig {
        resource_name: name.to_string(),
        name: api_name,
        api_type,
        routes: Vec::new(),
        route_selection_expression,
    }))
}

/// Parse aws_api_gateway_authorizer resource (v1)
fn parse_apigw_authorizer(name: &str, block: &hcl::Block) -> Option<ApigwAuthorizer> {
    let body = &block.body;
    Some(ApigwAuthorizer {
        resource_name: name.to_string(),
        rest_api_ref: get_traversal_attr(body, "rest_api_id").unwrap_or_default(),
        lambda_uri_ref: get_traversal_attr(body, "authorizer_uri"),
        auth_type: get_string_attr(body, "type").unwrap_or_else(|| "TOKEN".to_string()),
    })
}

/// Parse aws_apigatewayv2_authorizer resource (v2)
fn parse_apigwv2_authorizer(name: &str, block: &hcl::Block) -> Option<Apigwv2Authorizer> {
    let body = &block.body;
    Some(Apigwv2Authorizer {
        resource_name: name.to_string(),
        api_ref: get_traversal_attr(body, "api_id").unwrap_or_default(),
        lambda_uri_ref: get_traversal_attr(body, "authorizer_uri"),
        auth_type: get_string_attr(body, "authorizer_type")
            .unwrap_or_else(|| "REQUEST".to_string()),
    })
}

/// Parse aws_apigatewayv2_route resource
fn parse_apigatewayv2_route(name: &str, block: &hcl::Block) -> Option<Apigwv2Route> {
    let body = &block.body;
    Some(Apigwv2Route {
        resource_name: name.to_string(),
        api_ref: get_traversal_attr(body, "api_id").unwrap_or_default(),
        route_key: get_string_attr(body, "route_key")?,
        target_ref: get_traversal_attr(body, "target").or_else(|| get_string_attr(body, "target")),
        authorizer_ref: get_traversal_attr(body, "authorization_type")
            .and_then(|t| {
                if t == "NONE" {
                    None
                } else {
                    get_traversal_attr(body, "authorizer_id")
                }
            })
            .or_else(|| get_traversal_attr(body, "authorizer_id")),
    })
}

/// Parse aws_apigatewayv2_integration resource
fn parse_apigatewayv2_integration(name: &str, block: &hcl::Block) -> Option<Apigwv2Integration> {
    let body = &block.body;
    Some(Apigwv2Integration {
        resource_name: name.to_string(),
        api_ref: get_traversal_attr(body, "api_id").unwrap_or_default(),
        lambda_uri_ref: get_traversal_attr(body, "integration_uri"),
    })
}

/// Resolve API Gateway v2 routes
/// V2 is simpler: route_key = "GET /path" and target = "integrations/${integration_id}"
fn resolve_apigatewayv2_routes(
    config: &mut LambdaformConfig,
    routes: &[Apigwv2Route],
    integrations: &[Apigwv2Integration],
    authorizers: &[Apigwv2Authorizer],
) {
    // Build integration name → lambda resource lookup
    let integration_lambdas: HashMap<String, String> = integrations
        .iter()
        .filter_map(|i| {
            let lambda_name = extract_lambda_name_from_ref(i.lambda_uri_ref.as_deref()?);
            Some((i.resource_name.clone(), lambda_name))
        })
        .collect();

    for route in routes {
        // Parse route_key: "GET /users/{id}" or "$default"
        let (method, path) = parse_v2_route_key(&route.route_key);

        // Resolve target integration → lambda
        // target is like "integrations/${aws_apigatewayv2_integration.name.id}"
        // or via traversal: "aws_apigatewayv2_integration.name.id"
        let lambda_resource = route.target_ref.as_ref().and_then(|target| {
            // Try extracting integration name from the ref
            let parts: Vec<&str> = target.split('.').collect();
            if parts.len() >= 2 && parts[0] == "aws_apigatewayv2_integration" {
                integration_lambdas.get(parts[1]).cloned()
            } else {
                // Try matching by iterating integrations with same api_ref
                integrations
                    .iter()
                    .find(|i| {
                        // Match by api_ref
                        extract_resource_name_from_ref(&i.api_ref)
                            == extract_resource_name_from_ref(&route.api_ref)
                    })
                    .and_then(|i| integration_lambdas.get(&i.resource_name).cloned())
            }
        });

        let lambda_resource = match lambda_resource {
            Some(r) => r,
            None => continue,
        };

        // Resolve authorizer from route's authorizer_id ref
        let authorizer = route.authorizer_ref.as_ref().and_then(|ref_str| {
            let auth_name = extract_resource_name_from_ref(ref_str);
            authorizers
                .iter()
                .find(|a| a.resource_name == auth_name)
                .and_then(|a| {
                    let auth_type = match a.auth_type.as_str() {
                        "REQUEST" => AuthorizerType::Lambda,
                        "JWT" => return None, // JWT authorizers don't need Lambda execution
                        _ => AuthorizerType::Lambda,
                    };
                    let function_resource = a
                        .lambda_uri_ref
                        .as_ref()
                        .map(|uri| extract_lambda_name_from_ref(uri));
                    Some(AuthorizerConfig {
                        auth_type,
                        function_resource,
                    })
                })
        });

        let route_config = RouteConfig {
            method,
            path,
            function_resource: lambda_resource.clone(),
            authorizer,
        };

        // Find the matching v2 gateway
        let api_resource_name = extract_resource_name_from_ref(&route.api_ref);
        let gateway = config.gateways.iter_mut().find(|g| {
            g.resource_name == api_resource_name
                && (g.api_type == ApiType::Http || g.api_type == ApiType::WebSocket)
        });

        if let Some(gateway) = gateway {
            tracing::info!(
                "Resolved v2 route: {} {} → {}",
                route_config.method_str(),
                route_config.path,
                lambda_resource
            );
            gateway.routes.push(route_config);
        }
    }
}

/// Parse a v2 route key like "GET /users/{id}" into (HttpMethod, path)
/// For WebSocket APIs, route keys are: $connect, $disconnect, $default, or custom action names
fn parse_v2_route_key(route_key: &str) -> (HttpMethod, String) {
    match route_key {
        "$default" => (HttpMethod::Any, "/{proxy+}".to_string()),
        "$connect" | "$disconnect" => (HttpMethod::Any, route_key.to_string()),
        _ => {
            let parts: Vec<&str> = route_key.splitn(2, ' ').collect();
            if parts.len() == 2 {
                (parse_http_method(parts[0]), parts[1].to_string())
            } else {
                // Could be a custom WebSocket action name (e.g., "sendmessage")
                (HttpMethod::Any, route_key.to_string())
            }
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
fn parse_lambda_function(
    name: &str,
    block: &hcl::Block,
    resolver: &VariableResolver,
) -> Result<Option<LambdaConfig>> {
    let body = &block.body;

    // Extract required attributes (with variable resolution)
    let function_name = get_string_attr_resolved(body, "function_name", resolver)
        .unwrap_or_else(|| name.to_string());

    let handler = match get_string_attr_resolved(body, "handler", resolver) {
        Some(h) => h,
        None => {
            tracing::warn!("Lambda {} missing handler, skipping", name);
            return Ok(None);
        }
    };

    let runtime_str = match get_string_attr_resolved(body, "runtime", resolver) {
        Some(r) => r,
        None => {
            tracing::warn!("Lambda {} missing runtime, skipping", name);
            return Ok(None);
        }
    };

    // Extract optional attributes
    let timeout = get_number_attr(body, "timeout").unwrap_or(3);
    let memory_size = get_number_attr(body, "memory_size").unwrap_or(128);

    // Extract environment variables (with variable resolution)
    let environment = extract_environment_resolved(body, resolver);

    // Try to find source path
    let source_path = get_string_attr_resolved(body, "filename", resolver)
        .or_else(|| {
            get_string_attr(body, "source_code_hash")
                .and_then(|_| get_string_attr_resolved(body, "filename", resolver))
        })
        .map(std::path::PathBuf::from);

    // Extract layer references (e.g., [aws_lambda_layer_version.utils.arn])
    let layers = get_list_traversal_attrs(body, "layers")
        .into_iter()
        .filter_map(|ref_str| extract_layer_resource_name(&ref_str))
        .collect();

    Ok(Some(LambdaConfig {
        resource_name: name.to_string(),
        function_name,
        handler,
        runtime: Runtime::from_str(&runtime_str),
        source_path,
        environment,
        timeout,
        memory_size,
        layers,
    }))
}

/// Parse aws_lambda_layer_version resource
fn parse_lambda_layer(
    name: &str,
    block: &hcl::Block,
) -> Result<Option<crate::config::LayerConfig>> {
    let body = &block.body;

    let layer_name = get_string_attr(body, "layer_name").unwrap_or_else(|| name.to_string());

    // Source path from filename attribute
    let source_path = get_string_attr(body, "filename").map(std::path::PathBuf::from);

    // Compatible runtimes
    let compatible_runtimes = get_list_string_attrs(body, "compatible_runtimes");

    Ok(Some(crate::config::LayerConfig {
        resource_name: name.to_string(),
        layer_name,
        source_path,
        compatible_runtimes,
    }))
}

/// Parse aws_sfn_state_machine resource
fn parse_sfn_state_machine(
    name: &str,
    block: &hcl::Block,
) -> Result<Option<crate::config::StepFunctionConfig>> {
    let body = &block.body;

    let machine_name = get_string_attr(body, "name").unwrap_or_else(|| name.to_string());

    let machine_type = get_string_attr(body, "type").unwrap_or_else(|| "STANDARD".to_string());

    // Definition can be inline string or jsonencode()
    let definition = get_string_attr(body, "definition").unwrap_or_else(|| "{}".to_string());

    let role_arn_ref = get_string_attr(body, "role_arn");

    Ok(Some(crate::config::StepFunctionConfig {
        resource_name: name.to_string(),
        name: machine_name,
        machine_type,
        definition,
        role_arn_ref,
    }))
}

/// Parse aws_dynamodb_table resource
fn parse_dynamodb_table(
    name: &str,
    block: &hcl::Block,
    resolver: &VariableResolver,
) -> Result<Option<crate::config::DynamoDbTableConfig>> {
    let body = &block.body;

    let table_name =
        get_string_attr_resolved(body, "name", resolver).unwrap_or_else(|| name.to_string());

    let hash_key = get_string_attr(body, "hash_key");
    let range_key = get_string_attr(body, "range_key");
    let billing_mode =
        get_string_attr(body, "billing_mode").unwrap_or_else(|| "PROVISIONED".to_string());

    let stream_enabled = get_bool_attr(body, "stream_enabled").unwrap_or(false);

    // Parse GSI and LSI names from nested blocks
    let mut gsi_names = Vec::new();
    let mut lsi_names = Vec::new();

    for inner_block in body.blocks() {
        match inner_block.identifier.as_str() {
            "global_secondary_index" => {
                if let Some(idx_name) = get_string_attr(&inner_block.body, "name") {
                    gsi_names.push(idx_name);
                }
            }
            "local_secondary_index" => {
                if let Some(idx_name) = get_string_attr(&inner_block.body, "name") {
                    lsi_names.push(idx_name);
                }
            }
            _ => {}
        }
    }

    Ok(Some(crate::config::DynamoDbTableConfig {
        resource_name: name.to_string(),
        name: table_name,
        hash_key,
        range_key,
        billing_mode,
        gsi_names,
        lsi_names,
        stream_enabled,
    }))
}

/// Parse aws_sqs_queue resource
fn parse_sqs_queue(
    name: &str,
    block: &hcl::Block,
    resolver: &VariableResolver,
) -> Result<Option<crate::config::SqsQueueConfig>> {
    let body = &block.body;

    let queue_name =
        get_string_attr_resolved(body, "name", resolver).unwrap_or_else(|| name.to_string());

    let fifo_queue = get_bool_attr(body, "fifo_queue").unwrap_or(false);
    let visibility_timeout = get_number_attr(body, "visibility_timeout_seconds").unwrap_or(30);

    Ok(Some(crate::config::SqsQueueConfig {
        resource_name: name.to_string(),
        name: queue_name,
        fifo_queue,
        visibility_timeout,
    }))
}

/// Parse aws_sns_topic resource
fn parse_sns_topic(
    name: &str,
    block: &hcl::Block,
    resolver: &VariableResolver,
) -> Result<Option<crate::config::SnsTopicConfig>> {
    let body = &block.body;

    let topic_name =
        get_string_attr_resolved(body, "name", resolver).unwrap_or_else(|| name.to_string());

    let fifo_topic = get_bool_attr(body, "fifo_topic").unwrap_or(false);

    Ok(Some(crate::config::SnsTopicConfig {
        resource_name: name.to_string(),
        name: topic_name,
        fifo_topic,
    }))
}

/// Parse aws_lambda_event_source_mapping resource
fn parse_event_source_mapping(
    name: &str,
    block: &hcl::Block,
) -> Result<Option<crate::config::EventSourceMappingConfig>> {
    let body = &block.body;

    // event_source_arn is a traversal like aws_sqs_queue.my_queue.arn
    let source_ref = get_traversal_attr(body, "event_source_arn").unwrap_or_default();

    // function_name is a traversal like aws_lambda_function.processor.arn
    let function_ref = get_traversal_attr(body, "function_name").unwrap_or_default();

    // Determine source type from the reference
    let (source_type, source_resource) = if source_ref.starts_with("aws_sqs_queue.") {
        (
            crate::config::EventSourceType::Sqs,
            extract_resource_name_from_ref(&source_ref),
        )
    } else if source_ref.starts_with("aws_dynamodb_table.") {
        (
            crate::config::EventSourceType::DynamoDb,
            extract_resource_name_from_ref(&source_ref),
        )
    } else if source_ref.starts_with("aws_kinesis_stream.") {
        (
            crate::config::EventSourceType::Kinesis,
            extract_resource_name_from_ref(&source_ref),
        )
    } else {
        tracing::warn!(
            "Event source mapping '{}': unrecognized source '{}'",
            name,
            source_ref
        );
        return Ok(None);
    };

    let function_resource = extract_lambda_name_from_ref(&function_ref);
    let batch_size = get_number_attr(body, "batch_size").unwrap_or(10);
    let enabled = get_bool_attr(body, "enabled").unwrap_or(true);

    Ok(Some(crate::config::EventSourceMappingConfig {
        resource_name: name.to_string(),
        source_type,
        source_resource,
        function_resource,
        batch_size,
        enabled,
    }))
}

/// Extract resource name from a layer ARN reference like "aws_lambda_layer_version.utils.arn"
fn extract_layer_resource_name(ref_str: &str) -> Option<String> {
    let parts: Vec<&str> = ref_str.split('.').collect();
    if parts.len() >= 2 && parts[0] == "aws_lambda_layer_version" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// Get a list of traversal attributes (e.g., layers = [aws_lambda_layer_version.x.arn])
fn get_list_traversal_attrs(body: &hcl::Body, name: &str) -> Vec<String> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .map(|attr| match &attr.expr {
            hcl::Expression::Array(items) => items
                .iter()
                .filter_map(|item| match item {
                    hcl::Expression::Traversal(traversal) => {
                        let parts: Vec<String> = std::iter::once(traversal.expr.to_string())
                            .chain(traversal.operators.iter().map(|op| match op {
                                hcl::TraversalOperator::GetAttr(ident) => ident.to_string(),
                                hcl::TraversalOperator::Index(expr) => format!("{}", expr),
                                _ => String::new(),
                            }))
                            .collect();
                        Some(parts.join("."))
                    }
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

/// Get a list of string values from an attribute (e.g., compatible_runtimes = ["nodejs20.x"])
fn get_list_string_attrs(body: &hcl::Body, name: &str) -> Vec<String> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .map(|attr| match &attr.expr {
            hcl::Expression::Array(items) => items
                .iter()
                .filter_map(|item| match item {
                    hcl::Expression::String(s) => Some(s.to_string()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

/// Parse aws_api_gateway_rest_api resource
fn parse_api_gateway_rest(
    name: &str,
    block: &hcl::Block,
    resolver: &VariableResolver,
) -> Result<Option<ApiGatewayConfig>> {
    let body = &block.body;

    let api_name =
        get_string_attr_resolved(body, "name", resolver).unwrap_or_else(|| name.to_string());

    Ok(Some(ApiGatewayConfig {
        resource_name: name.to_string(),
        name: api_name,
        api_type: ApiType::Rest,
        routes: Vec::new(),
        route_selection_expression: None,
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
        .and_then(|attr| match &attr.expr {
            hcl::Expression::Traversal(traversal) => {
                let parts: Vec<String> = std::iter::once(traversal.expr.to_string())
                    .chain(traversal.operators.iter().map(|op| match op {
                        hcl::TraversalOperator::GetAttr(ident) => ident.to_string(),
                        hcl::TraversalOperator::Index(expr) => format!("{}", expr),
                        _ => String::new(),
                    }))
                    .collect();
                Some(parts.join("."))
            }
            hcl::Expression::String(s) => Some(s.to_string()),
            _ => None,
        })
}

/// Get a string attribute from HCL body (without variable resolution)
fn get_string_attr(body: &hcl::Body, name: &str) -> Option<String> {
    get_string_attr_resolved(body, name, &VariableResolver::default())
}

/// Get a string attribute from HCL body, resolving var.xxx references
fn get_string_attr_resolved(
    body: &hcl::Body,
    name: &str,
    resolver: &VariableResolver,
) -> Option<String> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .and_then(|attr| {
            match &attr.expr {
                hcl::Expression::String(s) => {
                    let resolved = resolver.resolve(s);
                    Some(resolved)
                }
                hcl::Expression::TemplateExpr(t) => {
                    let resolved = resolver.resolve(&t.to_string());
                    Some(resolved)
                }
                hcl::Expression::Traversal(traversal) => {
                    // Handle bare var.xxx references
                    let parts: Vec<String> = std::iter::once(traversal.expr.to_string())
                        .chain(traversal.operators.iter().map(|op| match op {
                            hcl::TraversalOperator::GetAttr(ident) => ident.to_string(),
                            hcl::TraversalOperator::Index(expr) => format!("{}", expr),
                            _ => String::new(),
                        }))
                        .collect();
                    let traversal_str = parts.join(".");
                    resolver.resolve_traversal(&traversal_str)
                }
                _ => None,
            }
        })
}

/// Get a boolean attribute from HCL body
fn get_bool_attr(body: &hcl::Body, name: &str) -> Option<bool> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .and_then(|attr| match &attr.expr {
            hcl::Expression::Bool(b) => Some(*b),
            hcl::Expression::String(s) => Some(s == "true"),
            _ => None,
        })
}

/// Get a number attribute from HCL body
fn get_number_attr(body: &hcl::Body, name: &str) -> Option<u32> {
    body.attributes()
        .find(|attr| attr.key.to_string() == name)
        .and_then(|attr| match &attr.expr {
            hcl::Expression::Number(n) => n.as_u64().map(|v| v as u32),
            _ => None,
        })
}

/// Extract environment variables from Lambda resource with variable resolution
fn extract_environment_resolved(
    body: &hcl::Body,
    resolver: &VariableResolver,
) -> HashMap<String, String> {
    let mut env = HashMap::new();

    for block in body.blocks() {
        let identifier = block.identifier.to_string();
        if identifier == "environment" {
            for attr in block.body.attributes() {
                if attr.key.to_string() == "variables" {
                    if let hcl::Expression::Object(obj) = &attr.expr {
                        for (key, value) in obj.iter() {
                            if let hcl::ObjectKey::Identifier(k) = key {
                                let resolved_value = match value {
                                    hcl::Expression::String(v) => Some(resolver.resolve(v)),
                                    hcl::Expression::TemplateExpr(t) => {
                                        Some(resolver.resolve(&t.to_string()))
                                    }
                                    hcl::Expression::Traversal(traversal) => {
                                        let parts: Vec<String> =
                                            std::iter::once(traversal.expr.to_string())
                                                .chain(traversal.operators.iter().map(
                                                    |op| match op {
                                                        hcl::TraversalOperator::GetAttr(ident) => {
                                                            ident.to_string()
                                                        }
                                                        hcl::TraversalOperator::Index(expr) => {
                                                            format!("{}", expr)
                                                        }
                                                        _ => String::new(),
                                                    },
                                                ))
                                                .collect();
                                        let traversal_str = parts.join(".");
                                        // Try var.xxx resolution first, fall back to
                                        // storing the raw traversal as a placeholder
                                        // for cross-resource resolution in post-parse step
                                        Some(
                                            resolver
                                                .resolve_traversal(&traversal_str)
                                                .unwrap_or_else(|| {
                                                    format!("${{{}}}", traversal_str)
                                                }),
                                        )
                                    }
                                    _ => None,
                                };
                                if let Some(val) = resolved_value {
                                    env.insert(k.to_string(), val);
                                }
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
        assert_eq!(
            lambda.environment.get("TABLE_NAME"),
            Some(&"my-table".to_string())
        );
    }

    #[test]
    fn test_parse_http_api_v2() {
        let tf_content = r#"
resource "aws_lambda_function" "hello" {
  function_name = "hello-http"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}

resource "aws_apigatewayv2_api" "api" {
  name          = "my-http-api"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_integration" "hello" {
  api_id           = aws_apigatewayv2_api.api.id
  integration_type = "AWS_PROXY"
  integration_uri  = aws_lambda_function.hello.invoke_arn
}

resource "aws_apigatewayv2_route" "get_hello" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "GET /hello"
  target    = aws_apigatewayv2_integration.hello.id
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "$default"
  target    = aws_apigatewayv2_integration.hello.id
}
"#;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.tf");
        fs::write(&file_path, tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();

        assert_eq!(config.functions.len(), 1);
        assert_eq!(config.gateways.len(), 1);

        let gw = &config.gateways[0];
        assert_eq!(gw.name, "my-http-api");
        assert_eq!(gw.api_type, ApiType::Http);
        assert_eq!(gw.routes.len(), 2);

        // GET /hello
        let r1 = &gw.routes[0];
        assert_eq!(r1.method, HttpMethod::Get);
        assert_eq!(r1.path, "/hello");
        assert_eq!(r1.function_resource, "hello");

        // $default → ANY /{proxy+} (catch-all)
        let r2 = &gw.routes[1];
        assert_eq!(r2.method, HttpMethod::Any);
        assert_eq!(r2.path, "/{proxy+}");
        assert_eq!(r2.function_resource, "hello");
    }

    #[test]
    fn test_parse_authorizer_v1() {
        let tf_content = r#"
resource "aws_lambda_function" "authorizer" {
  function_name = "my-authorizer"
  handler       = "auth.handler"
  runtime       = "nodejs20.x"
}

resource "aws_lambda_function" "protected" {
  function_name = "protected-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}

resource "aws_api_gateway_rest_api" "api" {
  name = "auth-test-api"
}

resource "aws_api_gateway_authorizer" "token_auth" {
  name            = "token-authorizer"
  rest_api_id     = aws_api_gateway_rest_api.api.id
  authorizer_uri  = aws_lambda_function.authorizer.invoke_arn
  type            = "TOKEN"
}

resource "aws_api_gateway_resource" "protected" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "protected"
}

resource "aws_api_gateway_method" "protected_get" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.protected.id
  http_method   = "GET"
  authorization = "CUSTOM"
  authorizer_id = aws_api_gateway_authorizer.token_auth.id
}

resource "aws_api_gateway_integration" "protected" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.protected.id
  http_method = aws_api_gateway_method.protected_get.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.protected.invoke_arn
}
"#;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.tf");
        fs::write(&file_path, tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();

        assert_eq!(config.functions.len(), 2);
        assert_eq!(config.gateways.len(), 1);

        let gw = &config.gateways[0];
        assert_eq!(gw.routes.len(), 1);

        let route = &gw.routes[0];
        assert_eq!(route.method, HttpMethod::Get);
        assert_eq!(route.path, "/protected");
        assert_eq!(route.function_resource, "protected");

        // Should have authorizer attached
        let auth = route
            .authorizer
            .as_ref()
            .expect("Route should have authorizer");
        assert_eq!(auth.auth_type, AuthorizerType::Lambda);
        assert_eq!(auth.function_resource, Some("authorizer".to_string()));
    }

    #[test]
    fn test_parse_lambda_layers() {
        let tf_content = r#"
resource "aws_lambda_layer_version" "utils" {
  layer_name          = "utils-layer"
  filename            = "layers/utils"
  compatible_runtimes = ["nodejs20.x", "nodejs18.x"]
}

resource "aws_lambda_layer_version" "common" {
  layer_name          = "common-layer"
  filename            = "layers/common"
  compatible_runtimes = ["nodejs20.x"]
}

resource "aws_lambda_function" "app" {
  function_name = "my-app"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "."

  layers = [
    aws_lambda_layer_version.utils.arn,
    aws_lambda_layer_version.common.arn
  ]
}
"#;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.tf");
        fs::write(&file_path, tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();

        // Should parse 2 layers
        assert_eq!(config.layers.len(), 2);

        let utils = &config.layers[0];
        assert_eq!(utils.resource_name, "utils");
        assert_eq!(utils.layer_name, "utils-layer");
        assert_eq!(
            utils.source_path,
            Some(std::path::PathBuf::from("layers/utils"))
        );
        assert_eq!(utils.compatible_runtimes, vec!["nodejs20.x", "nodejs18.x"]);

        let common = &config.layers[1];
        assert_eq!(common.resource_name, "common");
        assert_eq!(common.layer_name, "common-layer");

        // Function should reference both layers
        assert_eq!(config.functions.len(), 1);
        let func = &config.functions[0];
        assert_eq!(func.layers, vec!["utils", "common"]);
    }

    #[test]
    fn test_parse_dynamodb_tables() {
        let tf_content = r#"
resource "aws_dynamodb_table" "users" {
  name         = "users-table"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "userId"
  range_key    = "sortKey"

  attribute {
    name = "userId"
    type = "S"
  }

  attribute {
    name = "sortKey"
    type = "S"
  }

  attribute {
    name = "email"
    type = "S"
  }

  global_secondary_index {
    name            = "email-index"
    hash_key        = "email"
    projection_type = "ALL"
  }

  local_secondary_index {
    name            = "sort-by-date"
    range_key       = "createdAt"
    projection_type = "ALL"
  }

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"
}

resource "aws_dynamodb_table" "sessions" {
  name         = "sessions-table"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "sessionId"

  attribute {
    name = "sessionId"
    type = "S"
  }
}
"#;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.tf");
        fs::write(&file_path, tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();
        assert_eq!(config.dynamodb_tables.len(), 2);

        let users = &config.dynamodb_tables[0];
        assert_eq!(users.name, "users-table");
        assert_eq!(users.hash_key, Some("userId".to_string()));
        assert_eq!(users.range_key, Some("sortKey".to_string()));
        assert_eq!(users.billing_mode, "PAY_PER_REQUEST");
        assert_eq!(users.gsi_names, vec!["email-index"]);
        assert_eq!(users.lsi_names, vec!["sort-by-date"]);
        assert!(users.stream_enabled);

        let sessions = &config.dynamodb_tables[1];
        assert_eq!(sessions.name, "sessions-table");
        assert_eq!(sessions.hash_key, Some("sessionId".to_string()));
        assert_eq!(sessions.range_key, None);
        assert!(sessions.gsi_names.is_empty());
        assert!(!sessions.stream_enabled);
    }

    #[test]
    fn test_nested_apigw_v1_resources() {
        let tf_content = r#"
resource "aws_lambda_function" "users" {
  function_name = "users-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}

resource "aws_api_gateway_rest_api" "api" {
  name = "nested-api"
}

resource "aws_api_gateway_resource" "api_root" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "api"
}

resource "aws_api_gateway_resource" "v1" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_resource.api_root.id
  path_part   = "v1"
}

resource "aws_api_gateway_resource" "users" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_resource.v1.id
  path_part   = "users"
}

resource "aws_api_gateway_resource" "user_id" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_resource.users.id
  path_part   = "{id}"
}

resource "aws_api_gateway_method" "get_user" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.user_id.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_user" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.user_id.id
  http_method = aws_api_gateway_method.get_user.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.users.invoke_arn
}

resource "aws_api_gateway_method" "list_users" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.users.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "list_users" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.users.id
  http_method = aws_api_gateway_method.list_users.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.users.invoke_arn
}
"#;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.tf");
        fs::write(&file_path, tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();

        assert_eq!(config.gateways.len(), 1);
        let gw = &config.gateways[0];
        assert_eq!(gw.routes.len(), 2);

        // Sort routes by path for deterministic assertions
        let mut routes: Vec<_> = gw.routes.iter().collect();
        routes.sort_by_key(|r| r.path.clone());

        // GET /api/v1/users
        assert_eq!(routes[0].path, "/api/v1/users");
        assert_eq!(routes[0].method, HttpMethod::Get);
        assert_eq!(routes[0].function_resource, "users");

        // GET /api/v1/users/{id}
        assert_eq!(routes[1].path, "/api/v1/users/{id}");
        assert_eq!(routes[1].method, HttpMethod::Get);
        assert_eq!(routes[1].function_resource, "users");
    }

    #[test]
    fn test_opentofu_compatibility() {
        // OpenTofu uses identical HCL syntax with its own registry and features
        // like state encryption blocks. Lambdaform should parse these files fine.
        let tf_content = r#"
terraform {
  required_version = ">= 1.6.0"

  required_providers {
    aws = {
      source  = "registry.opentofu.org/hashicorp/aws"
      version = "~> 5.0"
    }
  }

  # OpenTofu-specific: state encryption
  encryption {
    key_provider "pbkdf2" "my_key" {
      passphrase = "test"
    }
    method "aes_gcm" "my_method" {
      keys = key_provider.pbkdf2.my_key
    }
    state {
      method   = method.aes_gcm.my_method
      enforced = false
    }
  }
}

variable "prefix" {
  type    = string
  default = "tofu"
}

locals {
  env = "dev"
}

resource "aws_lambda_function" "api" {
  function_name = "tofu-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = 30
  memory_size   = 256
  filename      = "api.zip"

  environment {
    variables = {
      ENV = "dev"
    }
  }
}

resource "aws_lambda_function" "worker" {
  function_name = "tofu-worker"
  handler       = "main.handler"
  runtime       = "python3.12"
  timeout       = 60
  filename      = "worker.zip"
}

resource "aws_api_gateway_rest_api" "api" {
  name = "tofu-api"
}

resource "aws_api_gateway_resource" "items" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "items"
}

resource "aws_api_gateway_method" "get_items" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.items.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_items" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  resource_id = aws_api_gateway_resource.items.id
  http_method = aws_api_gateway_method.get_items.http_method
  type        = "AWS_PROXY"
  uri         = aws_lambda_function.api.invoke_arn
}

resource "aws_apigatewayv2_api" "http" {
  name          = "tofu-http"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_integration" "worker" {
  api_id           = aws_apigatewayv2_api.http.id
  integration_type = "AWS_PROXY"
  integration_uri  = aws_lambda_function.worker.invoke_arn
}

resource "aws_apigatewayv2_route" "process" {
  api_id    = aws_apigatewayv2_api.http.id
  route_key = "POST /process"
  target    = aws_apigatewayv2_integration.worker.id
}

resource "aws_dynamodb_table" "data" {
  name         = "tofu-data"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }

  stream_enabled = true
}

resource "aws_sqs_queue" "tasks" {
  name                       = "tofu-tasks"
  visibility_timeout_seconds = 60
}

resource "aws_lambda_event_source_mapping" "sqs_worker" {
  event_source_arn = aws_sqs_queue.tasks.arn
  function_name    = aws_lambda_function.worker.arn
  batch_size       = 5
  enabled          = true
}
"#;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.tf");
        fs::write(&file_path, tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();

        // Should parse all resources despite OpenTofu-specific blocks
        assert_eq!(config.functions.len(), 2, "Should find 2 Lambda functions");
        assert_eq!(config.gateways.len(), 2, "Should find REST + HTTP gateways");
        assert_eq!(
            config.dynamodb_tables.len(),
            1,
            "Should find DynamoDB table"
        );
        assert_eq!(config.sqs_queues.len(), 1, "Should find SQS queue");
        assert_eq!(
            config.event_source_mappings.len(),
            1,
            "Should find event source mapping"
        );

        // Verify Lambda parsing
        let api = config
            .functions
            .iter()
            .find(|f| f.resource_name == "api")
            .unwrap();
        assert_eq!(api.function_name, "tofu-api");
        assert_eq!(api.runtime, Runtime::Nodejs20);
        assert_eq!(api.environment.get("ENV"), Some(&"dev".to_string()));

        let worker = config
            .functions
            .iter()
            .find(|f| f.resource_name == "worker")
            .unwrap();
        assert_eq!(worker.function_name, "tofu-worker");
        assert_eq!(worker.runtime, Runtime::Python312);

        // Verify REST API routes
        let rest_gw = config
            .gateways
            .iter()
            .find(|g| g.api_type == ApiType::Rest)
            .unwrap();
        assert_eq!(rest_gw.routes.len(), 1);
        assert_eq!(rest_gw.routes[0].path, "/items");
        assert_eq!(rest_gw.routes[0].method, HttpMethod::Get);

        // Verify HTTP API routes
        let http_gw = config
            .gateways
            .iter()
            .find(|g| g.api_type == ApiType::Http)
            .unwrap();
        assert_eq!(http_gw.routes.len(), 1);
        assert_eq!(http_gw.routes[0].path, "/process");
        assert_eq!(http_gw.routes[0].method, HttpMethod::Post);

        // Verify DynamoDB
        assert_eq!(config.dynamodb_tables[0].name, "tofu-data");
        assert!(config.dynamodb_tables[0].stream_enabled);

        // Verify SQS → Lambda mapping
        let esm = &config.event_source_mappings[0];
        assert_eq!(esm.source_resource, "tasks");
        assert_eq!(esm.function_resource, "worker");
        assert_eq!(esm.batch_size, 5);
    }

    #[test]
    fn test_variable_resolution_from_defaults() {
        let tf_content = r#"
variable "project" {
  type    = string
  default = "myapp"
}

variable "env" {
  type    = string
  default = "dev"
}

resource "aws_lambda_function" "api" {
  function_name = "${var.project}-${var.env}-api"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = 30

  environment {
    variables = {
      PROJECT = var.project
      ENV     = var.env
      MIXED   = "${var.project}-service"
    }
  }
}
"#;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.tf"), tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();
        assert_eq!(config.functions.len(), 1);

        let func = &config.functions[0];
        assert_eq!(func.function_name, "myapp-dev-api");
        assert_eq!(func.environment.get("PROJECT"), Some(&"myapp".to_string()));
        assert_eq!(func.environment.get("ENV"), Some(&"dev".to_string()));
        assert_eq!(
            func.environment.get("MIXED"),
            Some(&"myapp-service".to_string())
        );
    }

    #[test]
    fn test_variable_resolution_from_tfvars() {
        let tf_content = r#"
variable "project" {
  type    = string
  default = "default-name"
}

variable "region" {
  type = string
}

resource "aws_lambda_function" "worker" {
  function_name = var.project
  handler       = "main.handler"
  runtime       = "python3.12"

  environment {
    variables = {
      REGION = var.region
    }
  }
}
"#;

        let tfvars_content = r#"
project = "overridden-name"
region  = "us-west-2"
"#;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.tf"), tf_content).unwrap();
        fs::write(dir.path().join("terraform.tfvars"), tfvars_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();
        assert_eq!(config.functions.len(), 1);

        let func = &config.functions[0];
        // tfvars should override variable defaults
        assert_eq!(func.function_name, "overridden-name");
        assert_eq!(
            func.environment.get("REGION"),
            Some(&"us-west-2".to_string())
        );
    }

    #[test]
    fn test_auto_tfvars_override_order() {
        let tf_content = r#"
variable "stage" {
  type    = string
  default = "dev"
}

resource "aws_lambda_function" "api" {
  function_name = var.stage
  handler       = "index.handler"
  runtime       = "nodejs20.x"
}
"#;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.tf"), tf_content).unwrap();
        fs::write(
            dir.path().join("terraform.tfvars"),
            "stage = \"from-tfvars\"\n",
        )
        .unwrap();
        fs::write(dir.path().join("prod.auto.tfvars"), "stage = \"prod\"\n").unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();
        let func = &config.functions[0];
        // auto.tfvars should override terraform.tfvars
        assert_eq!(func.function_name, "prod");
    }

    #[test]
    fn test_variable_resolution_in_dynamodb_table_names() {
        let tf_content = r#"
variable "stage" {
  type    = string
  default = "dev"
}

resource "aws_dynamodb_table" "users" {
  name         = "myapp-users-${var.stage}"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "userId"

  attribute {
    name = "userId"
    type = "S"
  }
}

resource "aws_sqs_queue" "jobs" {
  name = "myapp-jobs-${var.stage}"
}
"#;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.tf"), tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();
        assert_eq!(config.dynamodb_tables.len(), 1);
        assert_eq!(config.dynamodb_tables[0].name, "myapp-users-dev");

        assert_eq!(config.sqs_queues.len(), 1);
        assert_eq!(config.sqs_queues[0].name, "myapp-jobs-dev");
    }

    #[test]
    fn test_cross_resource_env_var_resolution() {
        let tf_content = r#"
variable "stage" {
  type    = string
  default = "dev"
}

resource "aws_dynamodb_table" "meetings" {
  name         = "meetings-${var.stage}"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "meeting_id"

  attribute {
    name = "meeting_id"
    type = "S"
  }
}

resource "aws_sqs_queue" "ingest" {
  name = "ingest-queue-${var.stage}"
}

resource "aws_lambda_function" "api" {
  function_name = "api-${var.stage}"
  handler       = "index.handler"
  runtime       = "python3.12"

  environment {
    variables = {
      STAGE          = var.stage
      MEETINGS_TABLE = aws_dynamodb_table.meetings.name
      QUEUE_URL      = aws_sqs_queue.ingest.url
      QUEUE_ARN      = aws_sqs_queue.ingest.arn
    }
  }
}
"#;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.tf"), tf_content).unwrap();

        let config = parse_terraform_dir(dir.path()).unwrap();
        let func = &config.functions[0];

        assert_eq!(func.function_name, "api-dev");
        assert_eq!(func.environment.get("STAGE"), Some(&"dev".to_string()));
        assert_eq!(
            func.environment.get("MEETINGS_TABLE"),
            Some(&"meetings-dev".to_string())
        );
        assert!(func
            .environment
            .get("QUEUE_URL")
            .unwrap()
            .contains("ingest-queue-dev"));
        assert!(func
            .environment
            .get("QUEUE_ARN")
            .unwrap()
            .contains("ingest-queue-dev"));
    }

    #[test]
    fn test_local_module_support() {
        let fixture_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/local-modules");

        let config = parse_terraform_dir(&fixture_dir).unwrap();

        // Root-level function should be present
        let root_func = config
            .functions
            .iter()
            .find(|f| f.resource_name == "root_handler")
            .expect("root_handler should exist");
        assert_eq!(root_func.function_name, "root-handler");

        // Module function should be present with prefixed resource name
        let module_func = config
            .functions
            .iter()
            .find(|f| f.resource_name == "api.api_handler")
            .expect("api.api_handler should exist from module");
        // Variable override: environment=dev from parent, not default "prod"
        assert_eq!(module_func.function_name, "dev-api-handler");
        assert_eq!(
            module_func.environment.get("TABLE_NAME"),
            Some(&"users-table".to_string())
        );
        assert_eq!(
            module_func.environment.get("ENVIRONMENT"),
            Some(&"dev".to_string())
        );

        // DynamoDB table from module should be present
        let table = config
            .dynamodb_tables
            .iter()
            .find(|t| t.resource_name == "api.data")
            .expect("api.data table should exist from module");
        assert_eq!(table.name, "dev-data");
    }
}
