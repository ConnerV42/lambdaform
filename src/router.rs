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
            architecture: crate::config::Architecture::default(),
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

    fn make_gateway(routes: Vec<RouteConfig>) -> ApiGatewayConfig {
        ApiGatewayConfig {
            resource_name: "api".to_string(),
            name: "test-api".to_string(),
            api_type: ApiType::Rest,
            routes,
            route_selection_expression: None,
        }
    }

    #[test]
    fn test_method_mismatch_returns_none() {
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Post,
            "/items",
            "create_item",
        )]);
        let funcs = vec![make_test_lambda("create_item")];
        let router = Router::new(&[gw], &funcs);

        assert!(router.match_request(&HttpMethod::Get, "/items").is_none());
    }

    #[test]
    fn test_any_method_matches_all() {
        let gw = make_gateway(vec![make_test_route(HttpMethod::Any, "/proxy", "proxy_fn")]);
        let funcs = vec![make_test_lambda("proxy_fn")];
        let router = Router::new(&[gw], &funcs);

        assert!(router.match_request(&HttpMethod::Get, "/proxy").is_some());
        assert!(router.match_request(&HttpMethod::Post, "/proxy").is_some());
        assert!(router
            .match_request(&HttpMethod::Delete, "/proxy")
            .is_some());
    }

    #[test]
    fn test_proxy_plus_catch_all() {
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Any,
            "/api/{proxy+}",
            "catch_all",
        )]);
        let funcs = vec![make_test_lambda("catch_all")];
        let router = Router::new(&[gw], &funcs);

        let m = router.match_request(&HttpMethod::Get, "/api/foo/bar/baz");
        assert!(m.is_some());
        let m = m.unwrap();
        assert_eq!(m.path_params.get("proxy"), Some(&"foo/bar/baz".to_string()));
    }

    #[test]
    fn test_multiple_path_params() {
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Get,
            "/users/{userId}/posts/{postId}",
            "get_post",
        )]);
        let funcs = vec![make_test_lambda("get_post")];
        let router = Router::new(&[gw], &funcs);

        let m = router.match_request(&HttpMethod::Get, "/users/42/posts/99");
        assert!(m.is_some());
        let m = m.unwrap();
        assert_eq!(m.path_params.get("userId"), Some(&"42".to_string()));
        assert_eq!(m.path_params.get("postId"), Some(&"99".to_string()));
    }

    #[test]
    fn test_no_match_wrong_path() {
        let gw = make_gateway(vec![make_test_route(HttpMethod::Get, "/users", "list_fn")]);
        let funcs = vec![make_test_lambda("list_fn")];
        let router = Router::new(&[gw], &funcs);

        assert!(router.match_request(&HttpMethod::Get, "/posts").is_none());
        assert!(router
            .match_request(&HttpMethod::Get, "/users/extra")
            .is_none());
    }

    #[test]
    fn test_first_matching_route_wins() {
        let gw = make_gateway(vec![
            make_test_route(HttpMethod::Get, "/items/{id}", "specific"),
            make_test_route(HttpMethod::Get, "/items/{proxy+}", "catch_all"),
        ]);
        let funcs = vec![make_test_lambda("specific"), make_test_lambda("catch_all")];
        let router = Router::new(&[gw], &funcs);

        let m = router
            .match_request(&HttpMethod::Get, "/items/123")
            .unwrap();
        assert_eq!(m.function.resource_name, "specific");
    }

    #[test]
    fn test_resource_path_is_template() {
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Get,
            "/orders/{orderId}",
            "get_order",
        )]);
        let funcs = vec![make_test_lambda("get_order")];
        let router = Router::new(&[gw], &funcs);

        let m = router
            .match_request(&HttpMethod::Get, "/orders/abc")
            .unwrap();
        assert_eq!(m.resource_path, Some("/orders/{orderId}".to_string()));
    }

    #[test]
    fn test_api_type_propagated() {
        let gw = ApiGatewayConfig {
            resource_name: "http_api".to_string(),
            name: "http-api".to_string(),
            api_type: ApiType::Http,
            routes: vec![make_test_route(HttpMethod::Get, "/v2", "v2_fn")],
            route_selection_expression: None,
        };
        let funcs = vec![make_test_lambda("v2_fn")];
        let router = Router::new(&[gw], &funcs);

        let m = router.match_request(&HttpMethod::Get, "/v2").unwrap();
        assert_eq!(m.api_type, ApiType::Http);
    }

    #[test]
    fn test_authorizer_attached() {
        let route = RouteConfig {
            method: HttpMethod::Get,
            path: "/secure".to_string(),
            function_resource: "handler".to_string(),
            authorizer: Some(AuthorizerConfig {
                auth_type: AuthorizerType::Lambda,
                function_resource: Some("auth_fn".to_string()),
            }),
        };
        let gw = make_gateway(vec![route]);
        let funcs = vec![make_test_lambda("handler"), make_test_lambda("auth_fn")];
        let router = Router::new(&[gw], &funcs);

        let m = router.match_request(&HttpMethod::Get, "/secure").unwrap();
        assert!(m.authorizer_function.is_some());
        assert_eq!(m.authorizer_function.unwrap().resource_name, "auth_fn");
    }

    #[test]
    fn test_for_gateway_constructor() {
        let gw = make_gateway(vec![make_test_route(HttpMethod::Get, "/test", "fn1")]);
        let funcs = vec![make_test_lambda("fn1")];
        let router = Router::for_gateway(&gw, &funcs);

        assert!(router.match_request(&HttpMethod::Get, "/test").is_some());
    }

    #[test]
    fn test_multi_gateway_routes_isolated() {
        let gw1 = ApiGatewayConfig {
            resource_name: "api_users".to_string(),
            name: "users-api".to_string(),
            api_type: ApiType::Rest,
            routes: vec![make_test_route(HttpMethod::Get, "/users", "list_users")],
            route_selection_expression: None,
        };
        let gw2 = ApiGatewayConfig {
            resource_name: "api_orders".to_string(),
            name: "orders-api".to_string(),
            api_type: ApiType::Http,
            routes: vec![make_test_route(HttpMethod::Get, "/orders", "list_orders")],
            route_selection_expression: None,
        };
        let funcs = vec![
            make_test_lambda("list_users"),
            make_test_lambda("list_orders"),
        ];
        let router = Router::new(&[gw1, gw2], &funcs);

        let m1 = router.match_request(&HttpMethod::Get, "/users").unwrap();
        assert_eq!(m1.function.resource_name, "list_users");
        assert_eq!(m1.api_type, ApiType::Rest);

        let m2 = router.match_request(&HttpMethod::Get, "/orders").unwrap();
        assert_eq!(m2.function.resource_name, "list_orders");
        assert_eq!(m2.api_type, ApiType::Http);
    }

    #[test]
    fn test_empty_router_matches_nothing() {
        let funcs = vec![make_test_lambda("fn1")];
        let router = Router::new(&[], &funcs);

        assert!(router
            .match_request(&HttpMethod::Get, "/anything")
            .is_none());
    }

    #[test]
    fn test_root_path_match() {
        let gw = make_gateway(vec![make_test_route(HttpMethod::Get, "/", "root_fn")]);
        let funcs = vec![make_test_lambda("root_fn")];
        let router = Router::new(&[gw], &funcs);

        let m = router.match_request(&HttpMethod::Get, "/");
        assert!(m.is_some());
        assert_eq!(m.unwrap().function.resource_name, "root_fn");
        // Should NOT match deeper paths
        assert!(router.match_request(&HttpMethod::Get, "/foo").is_none());
    }

    #[test]
    fn test_missing_function_skips_route() {
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Get,
            "/orphan",
            "nonexistent_fn",
        )]);
        let funcs = vec![make_test_lambda("other_fn")];
        let router = Router::new(&[gw], &funcs);

        // Route compiles but function lookup fails → no match
        assert!(router.match_request(&HttpMethod::Get, "/orphan").is_none());
    }

    #[test]
    fn test_same_path_different_methods() {
        let gw = make_gateway(vec![
            make_test_route(HttpMethod::Get, "/items", "get_items"),
            make_test_route(HttpMethod::Post, "/items", "create_item"),
            make_test_route(HttpMethod::Delete, "/items", "delete_item"),
        ]);
        let funcs = vec![
            make_test_lambda("get_items"),
            make_test_lambda("create_item"),
            make_test_lambda("delete_item"),
        ];
        let router = Router::new(&[gw], &funcs);

        assert_eq!(
            router
                .match_request(&HttpMethod::Get, "/items")
                .unwrap()
                .function
                .resource_name,
            "get_items"
        );
        assert_eq!(
            router
                .match_request(&HttpMethod::Post, "/items")
                .unwrap()
                .function
                .resource_name,
            "create_item"
        );
        assert_eq!(
            router
                .match_request(&HttpMethod::Delete, "/items")
                .unwrap()
                .function
                .resource_name,
            "delete_item"
        );
        // Unregistered method
        assert!(router.match_request(&HttpMethod::Put, "/items").is_none());
    }

    #[test]
    fn test_trailing_slash_no_match() {
        // Strict path matching: /users should NOT match /users/
        let gw = make_gateway(vec![make_test_route(HttpMethod::Get, "/users", "list_fn")]);
        let funcs = vec![make_test_lambda("list_fn")];
        let router = Router::new(&[gw], &funcs);

        assert!(router.match_request(&HttpMethod::Get, "/users").is_some());
        // Trailing slash is a different path
        assert!(router.match_request(&HttpMethod::Get, "/users/").is_none());
    }

    #[test]
    fn test_url_encoded_path_param() {
        // URL-encoded values should be captured as-is (decoding is caller's job)
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Get,
            "/files/{name}",
            "get_file",
        )]);
        let funcs = vec![make_test_lambda("get_file")];
        let router = Router::new(&[gw], &funcs);

        let m = router
            .match_request(&HttpMethod::Get, "/files/hello%20world")
            .unwrap();
        assert_eq!(
            m.path_params.get("name"),
            Some(&"hello%20world".to_string())
        );
    }

    #[test]
    fn test_deeply_nested_params() {
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Get,
            "/a/{b}/c/{d}/e/{f}",
            "deep_fn",
        )]);
        let funcs = vec![make_test_lambda("deep_fn")];
        let router = Router::new(&[gw], &funcs);

        let m = router
            .match_request(&HttpMethod::Get, "/a/1/c/2/e/3")
            .unwrap();
        assert_eq!(m.path_params.get("b"), Some(&"1".to_string()));
        assert_eq!(m.path_params.get("d"), Some(&"2".to_string()));
        assert_eq!(m.path_params.get("f"), Some(&"3".to_string()));
        // Wrong depth shouldn't match
        assert!(router.match_request(&HttpMethod::Get, "/a/1/c/2").is_none());
    }

    #[test]
    fn test_proxy_plus_captures_empty_suffix() {
        // {proxy+} with just the prefix should still match (captures empty string)
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Any,
            "/api/{proxy+}",
            "proxy_fn",
        )]);
        let funcs = vec![make_test_lambda("proxy_fn")];
        let router = Router::new(&[gw], &funcs);

        // Single segment after prefix
        let m = router.match_request(&HttpMethod::Get, "/api/x").unwrap();
        assert_eq!(m.path_params.get("proxy"), Some(&"x".to_string()));

        // Empty after prefix — .* matches empty string
        let m = router.match_request(&HttpMethod::Get, "/api/");
        assert!(m.is_some());
    }

    #[test]
    fn test_special_chars_in_literal_segments() {
        // Regex special chars in literal path segments should be escaped
        let gw = make_gateway(vec![make_test_route(
            HttpMethod::Get,
            "/v1.0/items",
            "v1_fn",
        )]);
        let funcs = vec![make_test_lambda("v1_fn")];
        let router = Router::new(&[gw], &funcs);

        assert!(router
            .match_request(&HttpMethod::Get, "/v1.0/items")
            .is_some());
        // The dot should NOT match any character
        assert!(router
            .match_request(&HttpMethod::Get, "/v1X0/items")
            .is_none());
    }

    #[test]
    fn test_non_lambda_authorizer_not_attached() {
        // Cognito/JWT authorizers (non-Lambda) should not produce authorizer_function
        let route = RouteConfig {
            method: HttpMethod::Get,
            path: "/protected".to_string(),
            function_resource: "handler".to_string(),
            authorizer: Some(AuthorizerConfig {
                auth_type: AuthorizerType::Cognito,
                function_resource: None,
            }),
        };
        let gw = make_gateway(vec![route]);
        let funcs = vec![make_test_lambda("handler")];
        let router = Router::new(&[gw], &funcs);

        let m = router
            .match_request(&HttpMethod::Get, "/protected")
            .unwrap();
        assert!(m.authorizer_function.is_none());
    }
}
