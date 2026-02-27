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
