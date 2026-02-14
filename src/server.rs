//! HTTP server for API Gateway emulation

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

use crate::config::LambdaformConfig;
use crate::project_config::CorsConfig;
use crate::router::Router as LambdaRouter;
use crate::runtime::{DebugOptions, FunctionExecutor, LambdaEvent};

/// Shared application state (behind RwLock for hot reload)
pub struct AppState {
    pub inner: RwLock<AppStateInner>,
    pub source_dir: std::path::PathBuf,
    pub debug: Option<DebugOptions>,
}

/// The reloadable portion of app state
pub struct AppStateInner {
    pub router: LambdaRouter,
    pub config: LambdaformConfig,
}

impl AppState {
    pub fn new(config: LambdaformConfig, source_dir: std::path::PathBuf, debug: Option<DebugOptions>) -> Self {
        let router = LambdaRouter::new(&config.gateways, &config.functions);
        Self {
            inner: RwLock::new(AppStateInner { router, config }),
            source_dir,
            debug,
        }
    }

    /// Reload configuration from Terraform files
    pub async fn reload(&self) -> anyhow::Result<()> {
        let new_config = crate::parser::parse_terraform_dir(&self.source_dir)?;
        let new_router = LambdaRouter::new(&new_config.gateways, &new_config.functions);

        let mut inner = self.inner.write().await;
        let fn_count = new_config.functions.len();
        let route_count: usize = new_config.gateways.iter().map(|g| g.routes.len()).sum();
        inner.config = new_config;
        inner.router = new_router;

        tracing::info!(
            "🔄 Reloaded: {} functions, {} routes",
            fn_count,
            route_count
        );
        Ok(())
    }
}

/// Build a CorsLayer from config
fn build_cors_layer(cors_config: Option<&CorsConfig>) -> CorsLayer {
    let config = match cors_config {
        Some(c) => c.clone(),
        None => CorsConfig::default(),
    };

    let mut layer = CorsLayer::new();

    // Origins
    if config.allow_origins.iter().any(|o| o == "*") {
        layer = layer.allow_origin(Any);
    } else {
        let origins: Vec<axum::http::HeaderValue> = config.allow_origins.iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        layer = layer.allow_origin(origins);
    }

    // Methods
    if config.allow_methods.is_empty() {
        layer = layer.allow_methods(Any);
    } else {
        let methods: Vec<Method> = config.allow_methods.iter()
            .filter_map(|m| m.parse().ok())
            .collect();
        layer = layer.allow_methods(methods);
    }

    // Headers
    if config.allow_headers.is_empty() || config.allow_headers.iter().any(|h| h == "*") {
        layer = layer.allow_headers(Any);
    } else {
        let headers: Vec<axum::http::header::HeaderName> = config.allow_headers.iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        layer = layer.allow_headers(headers);
    }

    // Expose headers
    if !config.expose_headers.is_empty() {
        let headers: Vec<axum::http::header::HeaderName> = config.expose_headers.iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        layer = layer.expose_headers(headers);
    }

    // Credentials
    if config.allow_credentials {
        layer = layer.allow_credentials(true);
    }

    // Max age
    if let Some(max_age) = config.max_age {
        layer = layer.max_age(std::time::Duration::from_secs(max_age));
    }

    layer
}

/// Start the HTTP server
pub async fn start_server(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    port: u16,
    cors_config: Option<&CorsConfig>,
    debug: Option<DebugOptions>,
) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config, source_dir, debug));
    let cors = build_cors_layer(cors_config);

    let app = Router::new()
        .route("/*path", any(handle_request))
        .route("/", any(handle_request))
        .layer(cors)
        .with_state(state.clone());

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Start the HTTP server with hot reload watcher
pub async fn start_server_with_watch(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    port: u16,
    cors_config: Option<&CorsConfig>,
    debug: Option<DebugOptions>,
) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config, source_dir.clone(), debug));
    let cors = build_cors_layer(cors_config);

    // Start file watcher (hold handle to keep it alive)
    let watcher_state = state.clone();
    let watch_dir = source_dir.clone();
    let _watch_handle = start_watcher(watch_dir, watcher_state)?;

    let app = Router::new()
        .route("/*path", any(handle_request))
        .route("/", any(handle_request))
        .layer(cors)
        .with_state(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Start the file watcher in a background thread
fn start_watcher(
    dir: std::path::PathBuf,
    state: Arc<AppState>,
) -> anyhow::Result<crate::watcher::WatchHandle> {
    use crate::watcher::{FileChange, WatchConfig};

    let mut watch_config = WatchConfig::default();
    watch_config.watch_paths.push(dir);

    // We need a handle to the tokio runtime to spawn reload tasks
    let rt_handle = tokio::runtime::Handle::current();

    let handle = crate::watcher::start_watching(watch_config, move |change| {
        match &change {
            FileChange::Terraform(path) => {
                tracing::info!("📝 Terraform changed: {}", path.display());
                let state = state.clone();
                rt_handle.spawn(async move {
                    if let Err(e) = state.reload().await {
                        tracing::error!("❌ Reload failed: {}", e);
                    }
                });
            }
            FileChange::Source(path) => {
                tracing::info!(
                    "📝 Source changed: {} (will use on next invocation)",
                    path.display()
                );
            }
        }
    })?;

    tracing::info!("👀 Watching for file changes");
    Ok(handle)
}

/// Format a duration in human-readable form
fn format_duration(duration: std::time::Duration) -> String {
    let ms = duration.as_secs_f64() * 1000.0;
    if ms < 1.0 {
        format!("{:.0}µs", duration.as_micros())
    } else if ms < 1000.0 {
        format!("{:.1}ms", ms)
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}

/// Format body size in human-readable form
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    method: Method,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Response {
    let request_start = std::time::Instant::now();
    let path = format!("/{}", path);

    // Build request info for logging
    let query_str = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&"))
    };

    let body_size = body.len();
    let content_type = headers.get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();

    tracing::info!("→ {} {}{}{}", method, path, query_str,
        if body_size > 0 { format!(" [body: {}, type: {}]", format_bytes(body_size), content_type) } else { String::new() }
    );

    // Convert method to our enum
    let http_method = match method {
        Method::GET => crate::config::HttpMethod::Get,
        Method::POST => crate::config::HttpMethod::Post,
        Method::PUT => crate::config::HttpMethod::Put,
        Method::PATCH => crate::config::HttpMethod::Patch,
        Method::DELETE => crate::config::HttpMethod::Delete,
        Method::OPTIONS => crate::config::HttpMethod::Options,
        Method::HEAD => crate::config::HttpMethod::Head,
        _ => crate::config::HttpMethod::Any,
    };

    // Lock state for reading
    let inner = state.inner.read().await;

    // Match route
    let matched = match inner.router.match_request(&http_method, &path) {
        Some(m) => m,
        None => {
            let duration = request_start.elapsed();
            tracing::warn!("← ⚠️ 404 {} {}{} [{}] no matching route", method, path, query_str, format_duration(duration));
            let body = serde_json::json!({
                "message": format!("No route matched: {} {}", method, path),
                "hint": "Run `lambdaform config` to see available routes"
            });
            return (StatusCode::NOT_FOUND, body.to_string()).into_response();
        }
    };

    // Build resource path (with parameter placeholders)
    let resource_path = matched.resource_path.clone().unwrap_or_else(|| path.clone());

    // Build request context (matches real AWS API Gateway)
    let request_context = crate::runtime::RequestContext {
        stage: "local".to_string(),
        resource_path: resource_path.clone(),
        http_method: method.to_string(),
        request_id: format!("lambdaform-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()),
        api_id: "lambdaform".to_string(),
        path: path.clone(),
        identity: crate::runtime::RequestIdentity {
            source_ip: "127.0.0.1".to_string(),
        },
    };

    // Build Lambda event
    let event = LambdaEvent {
        http_method: method.to_string(),
        path: path.clone(),
        resource: resource_path,
        path_parameters: if matched.path_params.is_empty() {
            None
        } else {
            Some(matched.path_params)
        },
        query_string_parameters: if query.is_empty() { None } else { Some(query.clone()) },
        headers: Some(
            headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
        ),
        body: if body.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&body).to_string())
        },
        is_base64_encoded: false,
        request_context,
    };

    // Clone what we need before dropping the lock
    let function = matched.function.clone();
    let authorizer_function = matched.authorizer_function.cloned();
    drop(inner);

    // Execute authorizer if present
    if let Some(auth_fn) = authorizer_function {
        let auth_event = crate::runtime::AuthorizerEvent {
            auth_type: "TOKEN".to_string(),
            authorization_token: headers.get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            method_arn: format!("arn:aws:execute-api:local:000000000000:api/{}/{}", method, path),
            http_method: method.to_string(),
            path: path.clone(),
            headers: Some(headers.iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect()),
            query_string_parameters: if query.is_empty() { None } else { Some(query.clone()) },
        };

        let auth_executor = FunctionExecutor::new(auth_fn, state.source_dir.clone())
            .with_debug(state.debug.clone());
        match auth_executor.invoke_authorizer(auth_event).await {
            Ok(result) => {
                if !result.is_authorized {
                    let duration = request_start.elapsed();
                    tracing::warn!("← ⚠️ 401 {} {} [{}] authorizer denied", method, path, format_duration(duration));
                    let body = serde_json::json!({
                        "message": "Unauthorized",
                    });
                    return (StatusCode::UNAUTHORIZED, body.to_string()).into_response();
                }
                tracing::debug!("🔓 Authorizer allowed: {} {}", method, path);
            }
            Err(e) => {
                tracing::error!("Authorizer error: {}", e);
                let body = serde_json::json!({
                    "message": "Authorizer error",
                    "error": e.to_string(),
                });
                return (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response();
            }
        }
    }

    // Execute function
    let executor = FunctionExecutor::new(function.clone(), state.source_dir.clone())
        .with_debug(state.debug.clone());

    match executor.invoke(event).await {
        Ok(response) => {
            let duration = request_start.elapsed();
            let status = StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::OK);
            let response_body = response.body.unwrap_or_default();
            let response_size = response_body.len();

            let status_icon = if status.is_success() { "✅" }
                else if status.is_redirection() { "↪️" }
                else if status.is_client_error() { "⚠️" }
                else { "❌" };

            tracing::info!("← {} {} {} {} [{}] → {}",
                status_icon, status.as_u16(), method, path,
                format_duration(duration), format_bytes(response_size)
            );

            // Log slow requests
            if duration.as_millis() > 3000 {
                tracing::warn!("🐢 Slow request: {} {} took {}", method, path, format_duration(duration));
            }

            let mut builder = axum::response::Response::builder().status(response.status_code);

            if let Some(headers) = response.headers {
                for (key, value) in headers {
                    builder = builder.header(key, value);
                }
            }

            builder.body(response_body.into()).unwrap()
        }
        Err(e) => {
            let duration = request_start.elapsed();
            tracing::error!("← ❌ 500 {} {} [{}] error: {}", method, path, format_duration(duration), e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
