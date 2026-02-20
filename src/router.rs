//! API Gateway router
//!
//! Matches incoming HTTP requests to Lambda functions.

use crate::config::{ApiGatewayConfig, ApiType, HttpMethod, LambdaConfig, RouteConfig};
use regex::Regex;
use std::collections::HashMap;

/// Router that maps HTTP requests to Lambda functions
pub struct Router {
    /// Compiled route patterns
    routes: Vec<CompiledRoute>,

    /// Function lookup by resource name
    functions: HashMap<String, LambdaConfig>,
}

/// A compiled route pattern
struct CompiledRoute {
    /// Original path pattern (e.g., "/users/{id}")
    path_pattern: String,

    /// Compiled regex for matching
    regex: Regex,

    /// Parameter names in order
    param_names: Vec<String>,

    /// HTTP method
    method: HttpMethod,

    /// Target function resource name
    function_resource: String,

    /// Optional authorizer function resource name
    authorizer_function_resource: Option<String>,

    /// API type (REST v1 or HTTP v2)
    api_type: ApiType,
}

/// Route match result
pub struct RouteMatch<'a> {
    /// Matched Lambda function
    pub function: &'a LambdaConfig,

    /// Path parameters extracted from the request
    pub path_params: HashMap<String, String>,

    /// Optional authorizer Lambda function
    pub authorizer_function: Option<&'a LambdaConfig>,

    /// Original resource path template (e.g., /users/{id})
    pub resource_path: Option<String>,

    /// API type (REST v1 or HTTP v2)
    pub api_type: ApiType,
}

impl Router {
    /// Create a router for a single gateway
    pub fn for_gateway(gateway: &ApiGatewayConfig, functions: &[LambdaConfig]) -> Self {
        Self::new(std::slice::from_ref(gateway), functions)
    }

    /// Create a new router from configuration
    pub fn new(gateways: &[ApiGatewayConfig], functions: &[LambdaConfig]) -> Self {
        let mut routes = Vec::new();

        for gateway in gateways {
            for route in &gateway.routes {
                if let Some(mut compiled) = compile_route(route) {
                    compiled.api_type = gateway.api_type.clone();
                    // Attach authorizer function resource if it's a Lambda authorizer
                    compiled.authorizer_function_resource =
                        route.authorizer.as_ref().and_then(|a| {
                            if a.auth_type == crate::config::AuthorizerType::Lambda {
                                a.function_resource.clone()
                            } else {
                                None
                            }
                        });
                    routes.push(compiled);
                }
            }
        }

        let functions: HashMap<String, LambdaConfig> = functions
            .iter()
            .map(|f| (f.resource_name.clone(), f.clone()))
            .collect();

        Router { routes, functions }
    }

    /// Match a request to a Lambda function
    pub fn match_request(&self, method: &HttpMethod, path: &str) -> Option<RouteMatch<'_>> {
        for route in &self.routes {
            // Check method
            if route.method != *method && route.method != HttpMethod::Any {
                continue;
            }

            // Check path
            if let Some(captures) = route.regex.captures(path) {
                // Extract path parameters
                let mut path_params = HashMap::new();
                for (i, name) in route.param_names.iter().enumerate() {
                    if let Some(m) = captures.get(i + 1) {
                        path_params.insert(name.clone(), m.as_str().to_string());
                    }
                }

                // Look up function
                if let Some(function) = self.functions.get(&route.function_resource) {
                    let authorizer_function = route
                        .authorizer_function_resource
                        .as_ref()
                        .and_then(|name| self.functions.get(name));
                    return Some(RouteMatch {
                        function,
                        path_params,
                        authorizer_function,
                        resource_path: Some(route.path_pattern.clone()),
                        api_type: route.api_type.clone(),
                    });
                }
            }
        }

        None
    }
}

/// Compile a route pattern into a regex
fn compile_route(route: &RouteConfig) -> Option<CompiledRoute> {
    let mut regex_str = String::from("^");
    let mut param_names = Vec::new();

    // Convert path pattern to regex
    // /users/{id}/posts/{post_id} -> ^/users/([^/]+)/posts/([^/]+)$
    let parts: Vec<&str> = route.path.split('/').collect();

    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            regex_str.push('/');
        }

        if part.starts_with('{') && part.ends_with('}') {
            // Path parameter
            let name = &part[1..part.len() - 1];

            // Handle {proxy+} style catch-all
            if let Some(stripped) = name.strip_suffix('+') {
                param_names.push(stripped.to_string());
                regex_str.push_str("(.*)");
            } else {
                param_names.push(name.to_string());
                regex_str.push_str("([^/]+)");
            }
        } else {
            // Literal path segment
            regex_str.push_str(&regex::escape(part));
        }
    }

    regex_str.push('$');

    let regex = Regex::new(&regex_str).ok()?;

    Some(CompiledRoute {
        path_pattern: route.path.clone(),
        regex,
        param_names,
        method: route.method.clone(),
        function_resource: route.function_resource.clone(),
        authorizer_function_resource: None, // Set by Router::new after compilation
        api_type: ApiType::Rest,            // Overridden by Router::new with gateway's type
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn make_test_route(method: HttpMethod, path: &str, function: &str) -> RouteConfig {
        RouteConfig {
            method,
            path: path.to_string(),
            function_resource: function.to_string(),
            authorizer: None,
        }
    }

    fn make_test_lambda(name: &str) -> LambdaConfig {
        LambdaConfig {
            resource_name: name.to_string(),
            function_name: name.to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 3,
            memory_size: 128,
            layers: vec![],
        }
    }

    #[test]
    fn test_simple_route_match() {
        let routes = vec![RouteConfig {
            method: HttpMethod::Get,
            path: "/users".to_string(),
            function_resource: "list_users".to_string(),
            authorizer: None,
        }];

        let gateways = vec![ApiGatewayConfig {
            resource_name: "api".to_string(),
            name: "test-api".to_string(),
            api_type: ApiType::Rest,
            routes,
            route_selection_expression: None,
        }];

        let functions = vec![make_test_lambda("list_users")];

        let router = Router::new(&gateways, &functions);

        let matched = router.match_request(&HttpMethod::Get, "/users");
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().function.resource_name, "list_users");
    }

    #[test]
    fn test_path_parameter_extraction() {
        let routes = vec![make_test_route(HttpMethod::Get, "/users/{id}", "get_user")];

        let gateways = vec![ApiGatewayConfig {
            resource_name: "api".to_string(),
            name: "test-api".to_string(),
            api_type: ApiType::Rest,
            routes,
            route_selection_expression: None,
        }];

        let functions = vec![make_test_lambda("get_user")];

        let router = Router::new(&gateways, &functions);

        let matched = router.match_request(&HttpMethod::Get, "/users/123");
        assert!(matched.is_some());

        let m = matched.unwrap();
        assert_eq!(m.path_params.get("id"), Some(&"123".to_string()));
    }
}
