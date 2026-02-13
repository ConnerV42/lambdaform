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

use crate::config::LambdaformConfig;
use crate::router::Router as LambdaRouter;
use crate::runtime::{FunctionExecutor, LambdaEvent};

/// Shared application state (behind RwLock for hot reload)
pub struct AppState {
    pub inner: RwLock<AppStateInner>,
    pub source_dir: std::path::PathBuf,
}

/// The reloadable portion of app state
pub struct AppStateInner {
    pub router: LambdaRouter,
    pub config: LambdaformConfig,
}

impl AppState {
    pub fn new(config: LambdaformConfig, source_dir: std::path::PathBuf) -> Self {
        let router = LambdaRouter::new(&config.gateways, &config.functions);
        Self {
            inner: RwLock::new(AppStateInner { router, config }),
            source_dir,
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

/// Start the HTTP server
pub async fn start_server(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    port: u16,
) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config, source_dir));

    let app = Router::new()
        .route("/*path", any(handle_request))
        .route("/", any(handle_request))
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
) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config, source_dir.clone()));

    // Start file watcher (hold handle to keep it alive)
    let watcher_state = state.clone();
    let watch_dir = source_dir.clone();
    let _watch_handle = start_watcher(watch_dir, watcher_state)?;

    let app = Router::new()
        .route("/*path", any(handle_request))
        .route("/", any(handle_request))
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

/// Handle incoming HTTP requests
async fn handle_request(
    method: Method,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Response {
    let path = format!("/{}", path);

    tracing::info!("{} {}", method, path);

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
            tracing::warn!("No route matched: {} {}", method, path);
            let body = serde_json::json!({
                "message": format!("No route matched: {} {}", method, path),
                "hint": "Run `lambdaform config` to see available routes"
            });
            return (StatusCode::NOT_FOUND, body.to_string()).into_response();
        }
    };

    // Build Lambda event
    let event = LambdaEvent {
        http_method: method.to_string(),
        path: path.clone(),
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

        let auth_executor = FunctionExecutor::new(auth_fn, state.source_dir.clone());
        match auth_executor.invoke_authorizer(auth_event).await {
            Ok(result) => {
                if !result.is_authorized {
                    tracing::warn!("🔒 Authorizer denied: {} {}", method, path);
                    let body = serde_json::json!({
                        "message": "Unauthorized",
                    });
                    return (StatusCode::UNAUTHORIZED, body.to_string()).into_response();
                }
                tracing::info!("🔓 Authorizer allowed: {} {}", method, path);
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
    let executor = FunctionExecutor::new(function, state.source_dir.clone());

    match executor.invoke(event).await {
        Ok(response) => {
            let mut builder = axum::response::Response::builder().status(response.status_code);

            if let Some(headers) = response.headers {
                for (key, value) in headers {
                    builder = builder.header(key, value);
                }
            }

            let body = response.body.unwrap_or_default();
            builder.body(body.into()).unwrap()
        }
        Err(e) => {
            tracing::error!("Lambda execution error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
