//! Integration tests for Lambdaform
//!
//! Spins up a real server, sends HTTP requests via tower::ServiceExt, asserts responses.
//! No external HTTP client needed — tests run in-process.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
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
    assert!(body["message"].as_str().unwrap().contains("Hello from Lambdaform!"));
    assert_eq!(body["environment"], "local");
}

#[tokio::test]
async fn test_rest_get_hello_with_query() {
    let app = build_test_app("simple-node");
    let resp = app
        .oneshot(Request::get("/hello?name=Conner").body(Body::empty()).unwrap())
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
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
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
    assert!(body["message"].as_str().unwrap().contains("Hello from HTTP API!"));
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
async fn test_http_api_unmatched_returns_404() {
    let app = build_test_app("http-api");
    // Paths not matching any explicit route return 404
    // ($default route support may be added in a future version)
    let resp = app
        .oneshot(Request::get("/something/random").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
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
    assert!(body["message"].as_str().unwrap().contains("Hello from Lambdaform!"));
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
