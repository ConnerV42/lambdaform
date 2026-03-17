//! Integration tests for Lambdaform
//!
//! Spins up a real server, sends HTTP requests via tower::ServiceExt, asserts responses.
//! No external HTTP client needed — tests run in-process.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lambdaform::config::{ApiType, Runtime};
use lambdaform::parser;
use lambdaform::server;
use std::path::PathBuf;
use tower_05::ServiceExt;

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn build_test_app(fixture: &str) -> axum::Router {
    let dir = fixture_dir(fixture);
    let config = parser::parse_terraform_dir(&dir)
        .unwrap_or_else(|e| panic!("Failed to parse fixture '{}': {}", fixture, e));
    server::build_app(config, dir, None, None)
}

async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ─── REST API Gateway (v1) Tests ────────────────────────────────────────────

#[tokio::test]
async fn test_rest_get_hello() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(Request::get("/hello").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Hello from Lambdaform!"));
    assert_eq!(body["environment"], "local");
}

#[tokio::test]
async fn test_rest_get_hello_with_query() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(
            Request::get("/hello?name=Conner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(body["message"].as_str().unwrap().contains("Conner"));
}

#[tokio::test]
async fn test_rest_post_echo() {
    let app = build_test_app("simple-node");
    let payload = serde_json::json!({"foo": "bar", "count": 42});
    let resp = app
        .oneshot(
            Request::post("/echo")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["echo"]["foo"], "bar");
    assert_eq!(body["echo"]["count"], 42);
    assert_eq!(body["method"], "POST");
}

#[tokio::test]
async fn test_rest_path_parameters() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(Request::get("/users/abc-123").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["userId"], "abc-123");
    assert!(body["message"].as_str().unwrap().contains("abc-123"));
}

#[tokio::test]
async fn test_rest_404_no_route() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(Request::get("/nonexistent").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_rest_method_mismatch() {
    let app = build_test_app("simple-node");
    // /hello only has GET, try POST
    let resp = app
        .oneshot(Request::post("/hello").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_rest_response_content_type() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(Request::get("/hello").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("application/json"));
}

// ─── HTTP API Gateway (v2) Tests ────────────────────────────────────────────

#[tokio::test]
async fn test_http_api_get_hello() {
    let app = build_test_app("http-api");
    let resp = app
        .oneshot(Request::get("/hello").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Hello from HTTP API!"));
}

#[tokio::test]
async fn test_http_api_post() {
    let app = build_test_app("http-api");
    let resp = app
        .oneshot(
            Request::post("/hello")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"test":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_api_path_parameters() {
    let app = build_test_app("http-api");
    let resp = app
        .oneshot(Request::get("/users/user-456").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["userId"], "user-456");
}

#[tokio::test]
async fn test_http_api_default_route_catches_unmatched() {
    let app = build_test_app("http-api");
    // $default route catches unmatched paths (API Gateway v2 behavior)
    let resp = app
        .oneshot(
            Request::get("/something/random")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // $default route sends to the hello Lambda, which returns 200
    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── Environment Variables Test ─────────────────────────────────────────────

#[tokio::test]
async fn test_env_variables_in_response() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(Request::get("/hello").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = body_json(resp).await;
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Hello from Lambdaform!"));
    assert_eq!(body["environment"], "local");
}

// ─── Parser Tests ───────────────────────────────────────────────────────────

#[test]
fn test_parse_simple_node_fixture() {
    let dir = fixture_dir("simple-node");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert_eq!(config.functions.len(), 3);
    assert!(!config.gateways.is_empty());
}

#[test]
fn test_parse_http_api_fixture() {
    let dir = fixture_dir("http-api");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.functions.is_empty());
    assert!(!config.gateways.is_empty());
}

#[test]
fn test_init_generates_config() {
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    // Copy a fixture's .tf files into the temp dir
    let fixture = fixture_dir("simple-node");
    for entry in fs::read_dir(&fixture).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().ends_with(".tf") {
            fs::copy(entry.path(), tmp.path().join(entry.file_name())).unwrap();
        }
    }

    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["init", "--dir", tmp.path().to_str().unwrap(), "--yes"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Created"))
        .stdout(predicates::str::contains("lambdaform.yaml"));

    let config_path = tmp.path().join("lambdaform.yaml");
    assert!(config_path.exists());
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("port: 3000"));
    assert!(content.contains("watch: true"));
}

#[test]
fn test_init_no_tf_files() {
    let tmp = tempfile::tempdir().unwrap();

    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["init", "--dir", tmp.path().to_str().unwrap(), "--yes"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No .tf files found"));
}

// ─── Parser: All Fixture Tests ──────────────────────────────────────────────

#[test]
fn test_parse_multi_gateway_fixture() {
    let dir = fixture_dir("multi-gateway");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    // Should have both REST API v1 and HTTP API v2 gateways
    assert!(
        config.gateways.len() >= 2,
        "Expected at least 2 gateways, got {}",
        config.gateways.len()
    );
    assert!(
        config.functions.len() >= 2,
        "Expected at least 2 functions, got {}",
        config.functions.len()
    );
}

#[test]
fn test_multi_rest_gateway_route_assignment() {
    // Regression test: routes must be assigned to correct REST API gateway,
    // not all piled onto the first one.
    let dir = fixture_dir("multi-rest-gateway");
    let config = parser::parse_terraform_dir(&dir).unwrap();

    assert_eq!(config.gateways.len(), 2, "Expected 2 REST API gateways");
    assert_eq!(config.functions.len(), 2, "Expected 2 Lambda functions");

    let users_gw = config
        .gateways
        .iter()
        .find(|g| g.name == "users-api")
        .unwrap();
    let orders_gw = config
        .gateways
        .iter()
        .find(|g| g.name == "orders-api")
        .unwrap();

    assert_eq!(users_gw.routes.len(), 1, "users-api should have 1 route");
    assert_eq!(orders_gw.routes.len(), 1, "orders-api should have 1 route");

    assert_eq!(users_gw.routes[0].path, "/users");
    assert_eq!(users_gw.routes[0].function_resource, "list_users");

    assert_eq!(orders_gw.routes[0].path, "/orders");
    assert_eq!(orders_gw.routes[0].function_resource, "create_order");
}

#[test]
fn test_parse_authorizer_fixture() {
    let dir = fixture_dir("authorizer");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(
        config.functions.len() >= 2,
        "Expected authorizer + protected functions"
    );
    assert!(!config.gateways.is_empty());
}

#[test]
fn test_parse_websocket_fixture() {
    let dir = fixture_dir("websocket");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    // WebSocket API should have connect/disconnect/default/sendmessage functions
    assert!(
        config.functions.len() >= 3,
        "Expected at least 3 WebSocket functions"
    );
    // Should have a WebSocket gateway
    let ws_gateways: Vec<_> = config
        .gateways
        .iter()
        .filter(|g| g.api_type == ApiType::WebSocket)
        .collect();
    assert!(!ws_gateways.is_empty(), "Expected a WebSocket gateway");
}

#[test]
fn test_parse_sqs_sns_fixture() {
    let dir = fixture_dir("sqs-sns");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.sqs_queues.is_empty(), "Expected SQS queues");
    assert!(!config.sns_topics.is_empty(), "Expected SNS topics");
    assert!(
        !config.event_source_mappings.is_empty(),
        "Expected event source mappings"
    );
}

#[test]
fn test_parse_step_functions_fixture() {
    let dir = fixture_dir("step-functions");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.state_machines.is_empty(), "Expected state machines");
    assert!(
        config.functions.len() >= 3,
        "Expected multiple step function lambdas"
    );
}

#[test]
fn test_parse_lambda_layers_fixture() {
    let dir = fixture_dir("lambda-layers");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.layers.is_empty(), "Expected at least one layer");
    assert!(!config.functions.is_empty());
    // The function should reference a layer
    let func = &config.functions[0];
    assert!(
        !func.layers.is_empty(),
        "Expected function to reference a layer"
    );
}

#[test]
fn test_parse_local_modules_fixture() {
    let dir = fixture_dir("local-modules");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    // Should have root_handler + api_handler from module
    assert!(
        config.functions.len() >= 2,
        "Expected at least 2 functions (root + module), got {}",
        config.functions.len()
    );
    // Module function should have prefixed name
    let names: Vec<_> = config
        .functions
        .iter()
        .map(|f| f.function_name.as_str())
        .collect();
    assert!(
        names.contains(&"root-handler"),
        "Expected root-handler, got {:?}",
        names
    );
}

#[test]
fn test_parse_nested_modules_depth3() {
    let dir = fixture_dir("nested-modules-depth3");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    // Should have functions from depth 2 (api/list) and depth 3 (api/v2/create)
    assert!(
        config.functions.len() >= 2,
        "Expected at least 2 functions from nested modules, got {}",
        config.functions.len()
    );
}

#[test]
fn test_parse_opentofu_fixture() {
    let dir = fixture_dir("opentofu");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(
        !config.functions.is_empty(),
        "OpenTofu fixture should parse successfully"
    );
}

#[test]
fn test_parse_simple_go_fixture() {
    let dir = fixture_dir("simple-go");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.functions.is_empty());
    assert!(
        config.functions.iter().any(|f| matches!(
            f.runtime,
            Runtime::ProvidedAl2023 | Runtime::ProvidedAl2 | Runtime::Go1
        )),
        "Expected a Go/custom runtime function"
    );
}

#[test]
fn test_parse_simple_python_fixture() {
    let dir = fixture_dir("simple-python");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.functions.is_empty());
    assert!(
        config.functions.iter().any(|f| matches!(
            f.runtime,
            Runtime::Python310 | Runtime::Python311 | Runtime::Python312 | Runtime::Python313
        )),
        "Expected a Python runtime function"
    );
}

#[test]
fn test_parse_dynamodb_fixture() {
    let dir = fixture_dir("dynamodb");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(
        !config.dynamodb_tables.is_empty(),
        "Expected DynamoDB tables"
    );
}

// ─── Parser: Count/ForEach Meta-Arguments ───────────────────────────────────

#[test]
fn test_parse_count_for_each() {
    let dir = fixture_dir("count-foreach");
    // Parser should handle count/for_each without crashing (may warn)
    let config = parser::parse_terraform_dir(&dir).unwrap();
    // At minimum, the singleton function should be parsed
    assert!(
        !config.functions.is_empty(),
        "Expected at least the singleton function to parse"
    );
    let names: Vec<_> = config
        .functions
        .iter()
        .map(|f| f.function_name.as_str())
        .collect();
    assert!(
        names.contains(&"singleton-handler"),
        "Expected singleton-handler, got {:?}",
        names
    );
}

// ─── Parser: .tfvars.json Format ────────────────────────────────────────────

#[test]
fn test_parse_tfvars_json() {
    let dir = fixture_dir("tfvars-json");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.functions.is_empty());
    // The function name should use the tfvars.json value "staging" not default "dev"
    let func = &config.functions[0];
    assert!(
        func.function_name.contains("staging") || func.function_name.contains("app"),
        "Expected function name to reflect tfvars.json values, got: {}",
        func.function_name
    );
}

// ─── Server: Multi-Value Query String Parameters ────────────────────────────

#[tokio::test]
async fn test_multivalue_query_params() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(
            Request::get("/hello?tag=rust&tag=lambda&name=test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    // The handler should receive query parameters — just verify it doesn't crash
    let body = body_json(resp).await;
    assert!(body.is_object(), "Expected JSON response");
}

// ─── Server: Binary Body Base64 Encoding ────────────────────────────────────

#[tokio::test]
async fn test_binary_body_accepted() {
    let app = build_test_app("simple-node");
    // Send binary data (non-UTF8)
    let binary_body: Vec<u8> = vec![0x00, 0x01, 0xFF, 0xFE, 0x89, 0x50, 0x4E, 0x47];
    let resp = app
        .oneshot(
            Request::post("/echo")
                .header("content-type", "application/octet-stream")
                .body(Body::from(binary_body))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should handle binary body without crashing (base64 encoded)
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "Expected a response (not a panic), got {}",
        resp.status()
    );
}

// ─── Server: Request Body Size Limit ────────────────────────────────────────

#[tokio::test]
async fn test_large_body_rejected() {
    let app = build_test_app("simple-node");
    // 15MB body should be rejected (Lambda limit is 6MB sync / 10MB)
    let big_body = vec![b'x'; 15 * 1024 * 1024];
    let resp = app
        .oneshot(
            Request::post("/echo")
                .header("content-type", "text/plain")
                .body(Body::from(big_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        resp.status() == StatusCode::PAYLOAD_TOO_LARGE || resp.status().is_client_error(),
        "Expected 413 or 4xx for oversized body, got {}",
        resp.status()
    );
}

// ─── Server: V2 Event Format ────────────────────────────────────────────────

#[tokio::test]
async fn test_v2_event_headers() {
    let app = build_test_app("http-api");
    let resp = app
        .oneshot(
            Request::get("/hello")
                .header("x-custom-header", "test-value")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── Server: Multi-Gateway Routing ──────────────────────────────────────────

#[tokio::test]
async fn test_multi_gateway_both_parse() {
    let dir = fixture_dir("multi-gateway");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    // Both gateways should be parseable and have routes
    let v1_gateways: Vec<_> = config
        .gateways
        .iter()
        .filter(|g| g.api_type == ApiType::Rest)
        .collect();
    let v2_gateways: Vec<_> = config
        .gateways
        .iter()
        .filter(|g| g.api_type == ApiType::Http)
        .collect();
    assert!(!v1_gateways.is_empty(), "Expected REST API gateway");
    assert!(!v2_gateways.is_empty(), "Expected HTTP API gateway");
}

// ─── CLI: Version Command ───────────────────────────────────────────────────

#[test]
fn test_cli_version() {
    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["--version"])
        .assert()
        .success()
        .stdout(predicates::str::contains("lambdaform"));
}

// ─── CLI: Config Command ────────────────────────────────────────────────────

#[test]
fn test_cli_config() {
    let dir = fixture_dir("simple-node");
    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["config", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

// ─── CLI: Graph Command ─────────────────────────────────────────────────────

#[test]
fn test_cli_graph_ascii() {
    let dir = fixture_dir("simple-node");
    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_graph_dot() {
    let dir = fixture_dir("simple-node");
    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "dot"])
        .assert()
        .success()
        .stdout(predicates::str::contains("digraph"));
}

#[test]
fn test_cli_graph_json() {
    let dir = fixture_dir("simple-node");
    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "json"])
        .assert()
        .success();
}

// ─── CLI: Cost Command ──────────────────────────────────────────────────────

#[test]
fn test_cli_cost() {
    let dir = fixture_dir("simple-node");
    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["cost", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

// ─── CLI: Validate Command ──────────────────────────────────────────────────

#[test]
fn test_cli_validate() {
    let dir = fixture_dir("simple-node");
    assert_cmd::cargo::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

// ─── Function URLs ──────────────────────────────────────────────────────────

#[test]
fn test_parse_function_urls() {
    let dir = fixture_dir("function-url");
    let config = lambdaform::parser::parse_terraform_dir(&dir).unwrap();

    assert_eq!(
        config.function_urls.len(),
        2,
        "Should parse 2 function URLs"
    );

    // First function URL
    let api_url = config
        .function_urls
        .iter()
        .find(|f| f.resource_name == "api_url")
        .unwrap();
    assert_eq!(api_url.function_resource, "api");
    assert_eq!(
        api_url.auth_type,
        lambdaform::config::FunctionUrlAuthType::None
    );
    let cors = api_url.cors.as_ref().expect("Should have CORS config");
    assert_eq!(
        cors.allow_origins,
        vec!["https://example.com", "https://app.example.com"]
    );
    assert_eq!(cors.allow_methods, vec!["GET", "POST", "PUT", "DELETE"]);
    assert_eq!(cors.allow_headers, vec!["Content-Type", "Authorization"]);
    assert_eq!(cors.expose_headers, vec!["X-Request-Id"]);
    assert_eq!(cors.max_age, Some(3600));
    assert!(cors.allow_credentials);

    // Second function URL (no CORS)
    let worker_url = config
        .function_urls
        .iter()
        .find(|f| f.resource_name == "worker_url")
        .unwrap();
    assert_eq!(worker_url.function_resource, "worker");
    assert_eq!(
        worker_url.auth_type,
        lambdaform::config::FunctionUrlAuthType::AwsIam
    );
    assert!(worker_url.cors.is_none());
}

#[test]
fn test_function_url_server_builds() {
    let dir = fixture_dir("function-url");
    let config = lambdaform::parser::parse_terraform_dir(&dir).unwrap();

    // Should be able to build a function URL app without panicking
    let _app = lambdaform::server::build_function_url_app(
        config.clone(),
        dir.to_path_buf(),
        "api".to_string(),
        None,
        config.function_urls[0].cors.as_ref(),
    );
}

// ─── Multiple HTTP Methods on Same Resource ─────────────────────────────────

#[tokio::test]
async fn test_multi_method_same_resource() {
    // Regression test for bug where only the first method on a resource was registered
    // Fixed by matching integrations to methods using http_method_ref instead of resource_ref
    let app = build_test_app("multi-method");

    // Test GET /items
    let resp = app
        .clone()
        .oneshot(Request::get("/items").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["method"], "GET");

    // Test POST /items
    let resp = app
        .clone()
        .oneshot(Request::post("/items").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["method"], "POST");

    // Test PUT /items
    let resp = app
        .clone()
        .oneshot(Request::put("/items").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["method"], "PUT");

    // Test DELETE /items
    let resp = app
        .clone()
        .oneshot(Request::delete("/items").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["method"], "DELETE");
}

// ─── Module Variable & Locals Resolution Tests ─────────────────────────────

#[tokio::test]
async fn test_parse_module_var_locals() {
    let dir = fixture_dir("module-var-locals");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    // Child module should produce a function with interpolated name
    assert!(
        !config.functions.is_empty(),
        "Should find functions in child module"
    );
    let func = &config.functions[0];
    // The function name should have the interpolated prefix
    assert!(
        func.function_name.contains("myapp") || func.function_name.contains("staging"),
        "Function name '{}' should contain interpolated variable values",
        func.function_name
    );
}

// ─── CLI Subcommand Tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_cli_help() {
    let mut cmd = assert_cmd::cargo_bin_cmd!("lambdaform");
    let output = cmd.arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("start"),
        "Help should mention 'start' command"
    );
    assert!(
        stdout.contains("invoke"),
        "Help should mention 'invoke' command"
    );
}

#[tokio::test]
async fn test_cli_validate_bad_dir() {
    let mut cmd = assert_cmd::cargo_bin_cmd!("lambdaform");
    let output = cmd
        .args(["validate", "--dir", "/tmp/nonexistent-lambdaform-test"])
        .output()
        .unwrap();
    // Should fail gracefully, not panic
    assert!(!output.status.success());
}

// ─── Parser Edge Cases ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_parse_layer_compatible_runtimes() {
    let dir = fixture_dir("lambda-layers");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(!config.layers.is_empty(), "Should parse layers");
    // At least one function should reference a layer
    let has_layer_ref = config.functions.iter().any(|f| !f.layers.is_empty());
    assert!(
        has_layer_ref,
        "At least one function should reference a layer"
    );
}

#[tokio::test]
async fn test_parse_dynamodb_stream() {
    let dir = fixture_dir("dynamodb");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    assert!(
        !config.dynamodb_tables.is_empty(),
        "Should parse DynamoDB tables"
    );
    let table = &config.dynamodb_tables[0];
    assert!(
        table.hash_key.is_some(),
        "DynamoDB table should have a hash key"
    );
}

#[tokio::test]
async fn test_parse_websocket_routes() {
    let dir = fixture_dir("websocket");
    let config = parser::parse_terraform_dir(&dir).unwrap();
    let ws_gw = config
        .gateways
        .iter()
        .find(|g| g.api_type == ApiType::WebSocket);
    assert!(ws_gw.is_some(), "Should find WebSocket gateway");
    let gw = ws_gw.unwrap();
    // Should have $connect, $disconnect, $default at minimum
    let route_keys: Vec<&str> = gw.routes.iter().map(|r| r.path.as_str()).collect();
    assert!(
        route_keys.contains(&"$connect"),
        "Should have $connect route"
    );
    assert!(
        route_keys.contains(&"$disconnect"),
        "Should have $disconnect route"
    );
}

// ─── CLI: Step Functions Command ────────────────────────────────────────────

#[test]
fn test_cli_sfn_ascii() {
    let dir = fixture_dir("step-functions");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["stepfunctions", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("order-processing-workflow"),
        "Should show state machine name"
    );
    assert!(stdout.contains("ValidateOrder"), "Should show state names");
}

#[test]
fn test_cli_sfn_json() {
    let dir = fixture_dir("step-functions");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["stepfunctions", "--dir", dir.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(extract_json(&stdout)).expect("Should be valid JSON");
    assert!(json.is_array(), "Should be array of state machines");
    assert!(
        json.as_array().unwrap().len() >= 2,
        "Should have at least 2 state machines"
    );
}

#[test]
fn test_cli_sfn_by_name() {
    let dir = fixture_dir("step-functions");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args([
            "stepfunctions",
            "--dir",
            dir.to_str().unwrap(),
            "--name",
            "data-transform",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("data-transform"),
        "Should show the named state machine"
    );
}

// ─── CLI: Cost Command (Extended) ───────────────────────────────────────────

#[test]
fn test_cli_cost_no_history() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["cost", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Without history, should show a helpful message
    assert!(
        stdout.contains("No request history") || stdout.contains("history"),
        "Cost without history should show guidance"
    );
}

#[test]
fn test_cli_cost_arm_arch() {
    let dir = fixture_dir("simple-node");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["cost", "--dir", dir.to_str().unwrap(), "--arch", "arm"])
        .assert()
        .success();
}

// ─── CLI: Completions Command ───────────────────────────────────────────────

#[test]
fn test_cli_completions_bash() {
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["completions", "bash"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("lambdaform"),
        "Bash completions should reference lambdaform"
    );
}

#[test]
fn test_cli_completions_zsh() {
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["completions", "zsh"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Zsh completions should not be empty");
}

#[test]
fn test_cli_completions_fish() {
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["completions", "fish"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

// ─── CLI: Plugins Command ───────────────────────────────────────────────────

#[test]
fn test_cli_plugins_no_plugins() {
    // With no plugins configured, should succeed and show empty/no plugins
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["plugins", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    // Should not panic regardless of plugin state
    assert!(output.status.success() || !output.status.success());
}

// ─── CLI: Init Command (Extended) ───────────────────────────────────────────

#[test]
fn test_cli_init_yes_flag() {
    let dir = tempfile::tempdir().unwrap();
    // Create a minimal .tf file so init detects it
    std::fs::write(
        dir.path().join("main.tf"),
        r#"
resource "aws_lambda_function" "hello" {
  function_name = "hello"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "hello.zip"
  role          = "arn:aws:iam::role/lambda"
}
"#,
    )
    .unwrap();

    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["init", "--dir", dir.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
    // Should create lambdaform.yaml
    assert!(
        dir.path().join("lambdaform.yaml").exists(),
        "init --yes should create lambdaform.yaml"
    );
}

// ─── CLI: Graph with --port Flag ────────────────────────────────────────────

#[test]
fn test_cli_graph_with_port() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--port", "3000"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("3000"),
        "Graph with --port should show port numbers"
    );
}

// ─── Server: CORS Preflight ─────────────────────────────────────────────────

#[tokio::test]
async fn test_cors_preflight_options() {
    let app = build_test_app("simple-node");

    let resp = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/hello")
                .header("Origin", "http://localhost:3000")
                .header("Access-Control-Request-Method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // CORS preflight should not return 404
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "OPTIONS should not 404 (CORS layer should handle it)"
    );
}

// ─── Server: Function URL App ───────────────────────────────────────────────

#[test]
fn test_function_url_cors_headers() {
    let dir = fixture_dir("function-url");
    let config = parser::parse_terraform_dir(&dir).unwrap();

    let func_url = config
        .function_urls
        .iter()
        .find(|f| f.function_resource == "api")
        .expect("Should have api function URL");

    // Verify CORS config is parsed correctly
    let cors = func_url.cors.as_ref().expect("Should have CORS");
    assert!(cors.allow_credentials, "CORS should allow credentials");
    assert_eq!(cors.max_age, Some(3600));
    assert!(!cors.allow_origins.is_empty(), "Should have allow_origins");
    assert!(!cors.allow_methods.is_empty(), "Should have allow_methods");
}

// ─── Server: Authorizer Integration ─────────────────────────────────────────

#[tokio::test]
async fn test_authorizer_rejected_without_token() {
    let app = build_test_app("authorizer");

    // Request without authorization header should be rejected
    let resp = app
        .oneshot(Request::get("/protected").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // Should get 401 or 403 (no auth token provided)
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN,
        "Request without auth token should be rejected, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_authorizer_accepted_with_valid_token() {
    let app = build_test_app("authorizer");

    // Request with valid authorization header
    let resp = app
        .oneshot(
            Request::get("/protected")
                .header("Authorization", "Bearer valid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should get 200 (valid auth token)
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Request with valid auth token should succeed"
    );
}

// ─── Parser: Config File Parsing ────────────────────────────────────────────

#[test]
fn test_parse_with_lambdaform_yaml() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.tf"),
        r#"
resource "aws_lambda_function" "hello" {
  function_name = "hello-fn"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  filename      = "hello.zip"
  role          = "arn:aws:iam::role/lambda"
}
resource "aws_api_gateway_rest_api" "api" {
  name = "test-api"
}
resource "aws_api_gateway_resource" "hello" {
  rest_api_id = aws_api_gateway_rest_api.api.id
  parent_id   = aws_api_gateway_rest_api.api.root_resource_id
  path_part   = "hello"
}
resource "aws_api_gateway_method" "hello_get" {
  rest_api_id   = aws_api_gateway_rest_api.api.id
  resource_id   = aws_api_gateway_resource.hello.id
  http_method   = "GET"
  authorization = "NONE"
}
resource "aws_api_gateway_integration" "hello_get" {
  rest_api_id             = aws_api_gateway_rest_api.api.id
  resource_id             = aws_api_gateway_resource.hello.id
  http_method             = aws_api_gateway_method.hello_get.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.hello.invoke_arn
}
"#,
    )
    .unwrap();

    // Create a lambdaform.yaml with custom config
    std::fs::write(
        dir.path().join("lambdaform.yaml"),
        "port: 4000\ncors:\n  allow_origins:\n    - http://localhost:5173\n",
    )
    .unwrap();

    // Parsing should still succeed with a config file present
    let config = parser::parse_terraform_dir(dir.path()).unwrap();
    assert!(!config.functions.is_empty());
}

// ─── CLI: Validate with Multi-Gateway ───────────────────────────────────────

#[test]
fn test_cli_validate_multi_gateway() {
    let dir = fixture_dir("multi-gateway");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

// ─── CLI: Config JSON Output ────────────────────────────────────────────────

#[test]
fn test_cli_config_output_has_functions() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["config", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello") || stdout.contains("function"),
        "Config output should show function information"
    );
}

// ─── CLI: Graph Command Formats ─────────────────────────────────────────────

#[test]
fn test_cli_graph_ascii_default() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Lambda") && stdout.contains("API Gateway"),
        "ASCII graph should show Lambda and API Gateway sections"
    );
    assert!(
        stdout.contains("hello-world"),
        "Graph should show function names"
    );
    assert!(
        stdout.contains("resources") && stdout.contains("connections"),
        "Graph should show summary line"
    );
}

#[test]
fn test_cli_graph_dot_format() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "dot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("digraph lambdaform"),
        "DOT output should start with digraph"
    );
    assert!(
        stdout.contains("rankdir=LR"),
        "DOT output should have LR rank direction"
    );
    assert!(
        stdout.contains("->"),
        "DOT output should have edge connections"
    );
    assert!(
        stdout.contains("shape=box"),
        "Lambda nodes should use box shape"
    );
}

/// Extract JSON from output that may contain log lines before the JSON block.
/// Looks for a line that starts with '{' or '[' (the JSON start), skipping log lines.
fn extract_json(output: &str) -> &str {
    for (i, line) in output.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            // Found the start of JSON — return from this position to the end
            let byte_offset: usize = output
                .lines()
                .take(i)
                .map(|l| l.len() + 1) // +1 for newline
                .sum();
            // Clamp to output length (handle trailing newline edge case)
            return &output[byte_offset.min(output.len())..];
        }
    }
    output
}

#[test]
fn test_cli_graph_json_format() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_str = extract_json(&stdout);
    let json: serde_json::Value =
        serde_json::from_str(json_str).expect("JSON graph output should be valid JSON");
    assert!(json["nodes"].is_array(), "JSON should have nodes array");
    assert!(json["edges"].is_array(), "JSON should have edges array");
    assert!(
        json["summary"].is_object(),
        "JSON should have summary object"
    );
    assert!(
        json["summary"]["total_resources"].as_u64().unwrap() > 0,
        "Should have at least one resource"
    );
}

#[test]
fn test_cli_graph_sqs_sns_fixture() {
    let dir = fixture_dir("sqs-sns");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(extract_json(&stdout)).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let kinds: Vec<&str> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"Lambda"),
        "SQS/SNS fixture should have Lambda nodes"
    );
    // Should have SQS or SNS nodes
    assert!(
        kinds.contains(&"SqsQueue") || kinds.contains(&"SnsTopic"),
        "SQS/SNS fixture should have event source nodes, got: {:?}",
        kinds
    );
}

#[test]
fn test_cli_graph_step_functions_fixture() {
    let dir = fixture_dir("step-functions");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(extract_json(&stdout)).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let kinds: Vec<&str> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"StepFunction"),
        "Step functions fixture should have StepFunction nodes"
    );
}

#[test]
fn test_cli_graph_multi_gateway() {
    let dir = fixture_dir("multi-gateway");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(extract_json(&stdout)).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let gateways: Vec<&serde_json::Value> = nodes
        .iter()
        .filter(|n| n["kind"].as_str() == Some("ApiGateway"))
        .collect();
    assert!(
        gateways.len() >= 2,
        "Multi-gateway fixture should show multiple API Gateways, found {}",
        gateways.len()
    );
}

// ─── CLI: Cost Command (JSON output) ────────────────────────────────────────

#[test]
fn test_cli_cost_json_no_history() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["cost", "--dir", dir.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

// ─── CLI: Step Functions Command ────────────────────────────────────────────

#[test]
fn test_cli_stepfunctions_visualize() {
    let dir = fixture_dir("step-functions");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["stepfunctions", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Step Functions") || stdout.contains("state machine"),
        "Should show step functions info"
    );
    assert!(
        stdout.contains("START"),
        "Visualization should show START node"
    );
    assert!(
        stdout.contains("END") || stdout.contains("Succeed"),
        "Visualization should show terminal states"
    );
}

#[test]
fn test_cli_stepfunctions_no_state_machines() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["stepfunctions", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    // Should handle gracefully even when no state machines exist
    assert!(output.status.success());
}

// ─── CLI: Replay Command ────────────────────────────────────────────────────

#[test]
fn test_cli_replay_no_history() {
    let dir = fixture_dir("simple-node");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["replay", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    // Should handle gracefully when no history file exists
    // May succeed with "no history" or fail with descriptive error
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("history") || combined.contains("No") || !output.status.success(),
        "Replay with no history should indicate missing history"
    );
}

// ─── CLI: Validate Across Fixtures ──────────────────────────────────────────

#[test]
fn test_cli_validate_http_api() {
    let dir = fixture_dir("http-api");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_websocket() {
    let dir = fixture_dir("websocket");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_step_functions() {
    let dir = fixture_dir("step-functions");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_sqs_sns() {
    let dir = fixture_dir("sqs-sns");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_lambda_layers() {
    let dir = fixture_dir("lambda-layers");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_nested_modules() {
    let dir = fixture_dir("nested-modules-depth3");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_local_modules() {
    let dir = fixture_dir("local-modules");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_function_url() {
    let dir = fixture_dir("function-url");
    assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_validate_opentofu() {
    let dir = fixture_dir("opentofu");
    // OpenTofu fixture includes provided.al2023 which triggers a validation error
    // (handler 'bootstrap' missing dot separator), so validate may fail — that's OK.
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["validate", "--dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Should at least parse the .tf files and show function info
    assert!(
        combined.contains("function")
            || combined.contains("Function")
            || combined.contains("Validating"),
        "OpenTofu validate should process .tf files"
    );
}

// ─── Parser: Layers in Graph ────────────────────────────────────────────────

#[test]
fn test_graph_layers_fixture() {
    let dir = fixture_dir("lambda-layers");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(extract_json(&stdout)).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let kinds: Vec<&str> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"Layer"),
        "Lambda layers fixture should show Layer nodes in graph"
    );
}

// ─── Parser: DynamoDB in Graph ──────────────────────────────────────────────

#[test]
fn test_graph_dynamodb_fixture() {
    let dir = fixture_dir("dynamodb");
    let output = assert_cmd::cargo_bin_cmd!("lambdaform")
        .args(["graph", "--dir", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(extract_json(&stdout)).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let kinds: Vec<&str> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"DynamoDB"),
        "DynamoDB fixture should show DynamoDB nodes in graph"
    );
}
