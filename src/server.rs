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

use crate::config::ApiType;
use crate::config::{ApiGatewayConfig, LambdaConfig, LambdaformConfig};
use crate::history::HistoryRecorder;
use crate::pool::ProcessPool;
use crate::project_config::CorsConfig;
use crate::router::Router as LambdaRouter;
use crate::runtime::{DebugOptions, FunctionExecutor, LambdaEvent, LambdaEventV2};

/// Global plugin manager for custom resource handlers
static PLUGIN_MANAGER: std::sync::OnceLock<Arc<crate::plugin::PluginManager>> =
    std::sync::OnceLock::new();

/// Set the global plugin manager (call once before starting servers)
pub fn set_plugin_manager(pm: crate::plugin::PluginManager) {
    let _ = PLUGIN_MANAGER.set(Arc::new(pm));
}

/// Global TUI event sender for live request monitoring
#[cfg(feature = "tui")]
static TUI_SENDER: std::sync::OnceLock<
    tokio::sync::broadcast::Sender<crate::tui::ui::RequestEvent>,
> = std::sync::OnceLock::new();

/// Set the global TUI sender (call once before starting servers)
#[cfg(feature = "tui")]
pub fn set_tui_sender(tx: tokio::sync::broadcast::Sender<crate::tui::ui::RequestEvent>) {
    let _ = TUI_SENDER.set(tx);
}

/// Send a request event to the TUI (no-op if not set)
#[cfg(feature = "tui")]
fn emit_tui_event(event: crate::tui::ui::RequestEvent) {
    if let Some(tx) = TUI_SENDER.get() {
        let _ = tx.send(event);
    }
}

/// Gateway assignment: which gateway runs on which port
#[derive(Debug, Clone)]
pub struct GatewayBinding {
    pub gateway_name: String,
    pub gateway_resource: String,
    pub port: u16,
}

/// Shared application state (behind RwLock for hot reload)
pub struct AppState {
    pub inner: RwLock<AppStateInner>,
    pub source_dir: std::path::PathBuf,
    pub debug: Option<DebugOptions>,
    /// Which gateway this state serves (None = all gateways merged)
    pub gateway_resource: Option<String>,
    /// Warm process pool for Node.js/Python workers
    pub pool: Arc<ProcessPool>,
    /// Request history recorder
    pub history: Option<HistoryRecorder>,
    /// Port this server is bound to (for history recording)
    pub port: u16,
    /// Plugin manager for custom resource handlers
    pub plugin_manager: Option<Arc<crate::plugin::PluginManager>>,
}

/// The reloadable portion of app state
pub struct AppStateInner {
    pub router: LambdaRouter,
    pub config: LambdaformConfig,
}

impl AppState {
    /// Create state for all gateways merged (backward compat / single gateway)
    pub fn new(
        config: LambdaformConfig,
        source_dir: std::path::PathBuf,
        debug: Option<DebugOptions>,
    ) -> Self {
        let router = LambdaRouter::new(&config.gateways, &config.functions);
        Self {
            inner: RwLock::new(AppStateInner { router, config }),
            source_dir,
            debug,
            gateway_resource: None,
            pool: Arc::new(ProcessPool::new()),
            history: None,
            port: 0,
            plugin_manager: PLUGIN_MANAGER.get().cloned(),
        }
    }

    /// Set the plugin manager for this state
    pub fn with_plugins(mut self, manager: crate::plugin::PluginManager) -> Self {
        self.plugin_manager = Some(Arc::new(manager));
        self
    }

    /// Create state for a single gateway
    pub fn for_gateway(
        config: LambdaformConfig,
        gateway: &ApiGatewayConfig,
        source_dir: std::path::PathBuf,
        debug: Option<DebugOptions>,
    ) -> Self {
        let router = LambdaRouter::for_gateway(gateway, &config.functions);
        Self {
            inner: RwLock::new(AppStateInner { router, config }),
            source_dir,
            debug,
            gateway_resource: Some(gateway.resource_name.clone()),
            pool: Arc::new(ProcessPool::new()),
            history: None,
            port: 0,
            plugin_manager: PLUGIN_MANAGER.get().cloned(),
        }
    }

    /// Set the history recorder and port for this state
    pub fn with_history(mut self, history: HistoryRecorder, port: u16) -> Self {
        self.history = Some(history);
        self.port = port;
        self
    }

    /// Reload configuration from Terraform files
    pub async fn reload(&self) -> anyhow::Result<()> {
        let new_config = crate::parser::parse_terraform_dir(&self.source_dir)?;

        let new_router = if let Some(ref gw_resource) = self.gateway_resource {
            // Rebuild router for just this gateway
            if let Some(gw) = new_config
                .gateways
                .iter()
                .find(|g| g.resource_name == *gw_resource)
            {
                LambdaRouter::for_gateway(gw, &new_config.functions)
            } else {
                tracing::warn!("⚠️ Gateway '{}' not found after reload", gw_resource);
                LambdaRouter::new(&[], &new_config.functions)
            }
        } else {
            LambdaRouter::new(&new_config.gateways, &new_config.functions)
        };

        let mut inner = self.inner.write().await;
        let fn_count = new_config.functions.len();
        let route_count = match &self.gateway_resource {
            Some(res) => new_config
                .gateways
                .iter()
                .find(|g| g.resource_name == *res)
                .map(|g| g.routes.len())
                .unwrap_or(0),
            None => new_config.gateways.iter().map(|g| g.routes.len()).sum(),
        };
        inner.config = new_config;
        inner.router = new_router;

        tracing::info!(
            "🔄 Reloaded: {} functions, {} routes{}",
            fn_count,
            route_count,
            self.gateway_resource
                .as_ref()
                .map(|r| format!(" ({})", r))
                .unwrap_or_default()
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
        let origins: Vec<axum::http::HeaderValue> = config
            .allow_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        layer = layer.allow_origin(origins);
    }

    // Methods
    if config.allow_methods.is_empty() {
        layer = layer.allow_methods(Any);
    } else {
        let methods: Vec<Method> = config
            .allow_methods
            .iter()
            .filter_map(|m| m.parse().ok())
            .collect();
        layer = layer.allow_methods(methods);
    }

    // Headers
    if config.allow_headers.is_empty() || config.allow_headers.iter().any(|h| h == "*") {
        layer = layer.allow_headers(Any);
    } else {
        let headers: Vec<axum::http::header::HeaderName> = config
            .allow_headers
            .iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        layer = layer.allow_headers(headers);
    }

    // Expose headers
    if !config.expose_headers.is_empty() {
        let headers: Vec<axum::http::header::HeaderName> = config
            .expose_headers
            .iter()
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

/// Start the HTTP server (single port, all gateways merged)
/// Build the axum app without starting a listener (useful for testing)
pub fn build_app(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    cors_config: Option<&CorsConfig>,
    debug: Option<DebugOptions>,
) -> Router {
    let state = Arc::new(AppState::new(config, source_dir, debug));
    let cors = build_cors_layer(cors_config);

    Router::new()
        .route("/*path", any(handle_request))
        .route("/", any(handle_request))
        .layer(cors)
        .with_state(state)
}

/// Create a future that resolves on Ctrl+C (SIGINT).
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("🛑 Received Ctrl+C — shutting down gracefully...");
}

pub async fn start_server(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    port: u16,
    cors_config: Option<&CorsConfig>,
    debug: Option<DebugOptions>,
) -> anyhow::Result<()> {
    let history = HistoryRecorder::new(&source_dir).ok();
    if let Some(ref h) = history {
        tracing::info!(
            "📝 Recording request history to {}",
            h.file_path().display()
        );
    }
    let mut state = AppState::new(config, source_dir, debug);
    if let Some(h) = history {
        state = state.with_history(h, port);
    }
    let state = Arc::new(state);
    let pool = state.pool.clone();
    let cors = build_cors_layer(cors_config);

    let app = Router::new()
        .route("/*path", any(handle_request))
        .route("/", any(handle_request))
        .layer(cors)
        .with_state(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Clean up worker processes
    tracing::info!("Cleaning up worker processes...");
    pool.invalidate_all().await;
    tracing::info!("✅ Shutdown complete");

    Ok(())
}

/// Start multiple servers, one per gateway binding
pub async fn start_multi_gateway(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    bindings: Vec<GatewayBinding>,
    watch: bool,
    cors_config: Option<&CorsConfig>,
    debug: Option<DebugOptions>,
) -> anyhow::Result<()> {
    let mut handles = Vec::new();
    let mut watch_handles = Vec::new();
    let mut pools: Vec<Arc<ProcessPool>> = Vec::new();

    // Shared shutdown notify
    let (shutdown_tx, _) = tokio::sync::watch::channel(false);

    for binding in &bindings {
        let gateway = config
            .gateways
            .iter()
            .find(|g| g.resource_name == binding.gateway_resource)
            .ok_or_else(|| anyhow::anyhow!("Gateway '{}' not found", binding.gateway_resource))?;

        let history = HistoryRecorder::new(&source_dir).ok();
        let mut gw_state =
            AppState::for_gateway(config.clone(), gateway, source_dir.clone(), debug.clone());
        if let Some(h) = history {
            gw_state = gw_state.with_history(h, binding.port);
        }
        let state = Arc::new(gw_state);
        pools.push(state.pool.clone());

        if watch {
            let wh = start_watcher(source_dir.clone(), state.clone())?;
            watch_handles.push(wh);
        }

        let cors = build_cors_layer(cors_config);
        let app = Router::new()
            .route("/*path", any(handle_request))
            .route("/", any(handle_request))
            .layer(cors)
            .with_state(state);

        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], binding.port));
        let gw_name = binding.gateway_name.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
                anyhow::anyhow!("Failed to bind {} on port {}: {}", gw_name, addr.port(), e)
            })?;
            tracing::info!("🌐 {} listening on http://{}", gw_name, addr);
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
                .map_err(|e| anyhow::anyhow!("Server error for {}: {}", gw_name, e))
        });

        handles.push(handle);
    }

    // Wait for Ctrl+C, then signal all servers to stop
    shutdown_signal().await;
    let _ = shutdown_tx.send(true);

    // Wait for all servers to finish
    for handle in handles {
        let _ = handle.await;
    }

    // Clean up all worker pools
    tracing::info!("Cleaning up worker processes...");
    for pool in &pools {
        pool.invalidate_all().await;
    }
    tracing::info!("✅ Shutdown complete");

    Ok(())
}

/// Start the HTTP server with hot reload watcher (single port, all gateways merged)
pub async fn start_server_with_watch(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    port: u16,
    cors_config: Option<&CorsConfig>,
    debug: Option<DebugOptions>,
) -> anyhow::Result<()> {
    let history = HistoryRecorder::new(&source_dir).ok();
    if let Some(ref h) = history {
        tracing::info!(
            "📝 Recording request history to {}",
            h.file_path().display()
        );
    }
    let mut app_state = AppState::new(config, source_dir.clone(), debug);
    if let Some(h) = history {
        app_state = app_state.with_history(h, port);
    }
    let state = Arc::new(app_state);
    let pool = state.pool.clone();
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
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Clean up worker processes
    tracing::info!("Cleaning up worker processes...");
    pool.invalidate_all().await;
    tracing::info!("✅ Shutdown complete");

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

    let handle = crate::watcher::start_watching(watch_config, move |change| match &change {
        FileChange::Terraform(path) => {
            tracing::info!("📝 Terraform changed: {}", path.display());
            let state = state.clone();
            rt_handle.spawn(async move {
                state.pool.invalidate_all().await;
                if let Err(e) = state.reload().await {
                    tracing::error!("❌ Reload failed: {}", e);
                }
            });
        }
        FileChange::Source(path) => {
            tracing::info!(
                "📝 Source changed: {} — killing warm workers",
                path.display()
            );
            let pool = state.pool.clone();
            rt_handle.spawn(async move {
                pool.invalidate_all().await;
            });
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

/// Format current time as ISO 8601 (UTC)
fn format_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Simple UTC timestamp without chrono dependency
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Calculate date from days since epoch (civil_from_days algorithm)
    let z = days as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hours, minutes, seconds
    )
}

/// Handle incoming HTTP requests
async fn handle_request(
    method: Method,
    path_param: Option<Path<String>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Response {
    let request_start = std::time::Instant::now();
    let path = match path_param {
        Some(Path(p)) => format!("/{}", p),
        None => "/".to_string(),
    };

    // Build request info for logging
    let query_str = if query.is_empty() {
        String::new()
    } else {
        format!(
            "?{}",
            query
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&")
        )
    };

    let body_size = body.len();
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();

    tracing::info!(
        http.method = %method,
        http.path = %path,
        http.query = %query_str,
        http.body_bytes = body_size,
        http.content_type = %content_type,
        "→ {} {}{}{}",
        method,
        path,
        query_str,
        if body_size > 0 {
            format!(
                " [body: {}, type: {}]",
                format_bytes(body_size),
                content_type
            )
        } else {
            String::new()
        }
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
            tracing::warn!(
                "← ⚠️ 404 {} {}{} [{}] no matching route",
                method,
                path,
                query_str,
                format_duration(duration)
            );
            let body = serde_json::json!({
                "message": format!("No route matched: {} {}", method, path),
                "hint": "Run `lambdaform config` to see available routes"
            });
            return (StatusCode::NOT_FOUND, body.to_string()).into_response();
        }
    };

    // Build resource path (with parameter placeholders)
    let resource_path = matched
        .resource_path
        .clone()
        .unwrap_or_else(|| path.clone());

    let request_id = format!(
        "lambdaform-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );

    let headers_map: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let path_params = if matched.path_params.is_empty() {
        None
    } else {
        Some(matched.path_params)
    };

    let query_params = if query.is_empty() {
        None
    } else {
        Some(query.clone())
    };

    let body_str = if body.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&body).to_string())
    };

    // Build event based on API type (v1 REST vs v2 HTTP)
    let event: serde_json::Value = match matched.api_type {
        ApiType::Http => {
            // API Gateway v2 (HTTP API) event format
            let route_key = format!("{} {}", method, resource_path);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            // Extract cookies from Cookie header
            let cookies = headers_map.get("cookie").map(|cookie_header| {
                cookie_header
                    .split(';')
                    .map(|c| c.trim().to_string())
                    .collect::<Vec<String>>()
            });
            let event_v2 = LambdaEventV2 {
                version: "2.0".to_string(),
                route_key: route_key.clone(),
                raw_path: path.clone(),
                raw_query_string: query
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&"),
                cookies,
                path_parameters: path_params,
                query_string_parameters: query_params,
                stage_variables: None,
                headers: Some(headers_map),
                body: body_str,
                is_base64_encoded: false,
                request_context: crate::runtime::RequestContextV2 {
                    stage: "$default".to_string(),
                    request_id: request_id.clone(),
                    api_id: "lambdaform".to_string(),
                    route_key,
                    account_id: "123456789012".to_string(),
                    domain_name: "localhost".to_string(),
                    domain_prefix: "lambdaform".to_string(),
                    time: format_timestamp(),
                    time_epoch: now.as_millis() as u64,
                    http: crate::runtime::RequestContextHttp {
                        method: method.to_string(),
                        path: path.clone(),
                        protocol: "HTTP/1.1".to_string(),
                        source_ip: "127.0.0.1".to_string(),
                        user_agent: headers
                            .get("user-agent")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("lambdaform/local")
                            .to_string(),
                    },
                },
            };
            serde_json::to_value(event_v2).unwrap_or_default()
        }
        _ => {
            // API Gateway v1 (REST API) event format
            let request_context = crate::runtime::RequestContext {
                stage: "local".to_string(),
                resource_path: resource_path.clone(),
                http_method: method.to_string(),
                request_id,
                api_id: "lambdaform".to_string(),
                path: path.clone(),
                identity: crate::runtime::RequestIdentity {
                    source_ip: "127.0.0.1".to_string(),
                },
            };
            let event_v1 = LambdaEvent {
                http_method: method.to_string(),
                path: path.clone(),
                resource: resource_path,
                path_parameters: path_params,
                query_string_parameters: query_params,
                headers: Some(headers_map),
                body: body_str,
                is_base64_encoded: false,
                request_context,
            };
            serde_json::to_value(event_v1).unwrap_or_default()
        }
    };

    // Clone what we need before dropping the lock
    let function = matched.function.clone();
    let authorizer_function = matched.authorizer_function.cloned();
    let layers_config = inner.config.layers.clone();
    let archive_files = inner.config.archive_files.clone();
    drop(inner);

    // Execute authorizer if present
    if let Some(auth_fn) = authorizer_function {
        let auth_event = crate::runtime::AuthorizerEvent {
            auth_type: "TOKEN".to_string(),
            authorization_token: headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            method_arn: format!(
                "arn:aws:execute-api:local:000000000000:api/{}/{}",
                method, path
            ),
            http_method: method.to_string(),
            path: path.clone(),
            headers: Some(
                headers
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect(),
            ),
            query_string_parameters: if query.is_empty() {
                None
            } else {
                Some(query.clone())
            },
        };

        let auth_source_dir =
            auth_fn.resolve_source_dir_with_archives(&state.source_dir, &archive_files);
        let auth_executor = FunctionExecutor::new(auth_fn, auth_source_dir)
            .with_debug(state.debug.clone())
            .with_pool(Some(state.pool.clone()));
        match auth_executor.invoke_authorizer(auth_event).await {
            Ok(result) => {
                if !result.is_authorized {
                    let duration = request_start.elapsed();
                    tracing::warn!(
                        "← ⚠️ 401 {} {} [{}] authorizer denied",
                        method,
                        path,
                        format_duration(duration)
                    );
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

    // Resolve layer paths for this function
    let layer_paths = resolve_layer_paths(&function, &layers_config, &state.source_dir);

    // Execute function — resolve source directory per-function
    let fn_source_dir =
        function.resolve_source_dir_with_archives(&state.source_dir, &archive_files);
    let executor = FunctionExecutor::new(function.clone(), fn_source_dir)
        .with_debug(state.debug.clone())
        .with_pool(Some(state.pool.clone()))
        .with_layer_paths(layer_paths);

    // Capture request data for history recording (before invoke consumes things)
    let history_method = method.to_string();
    let history_path = path.clone();
    let history_query = if query.is_empty() {
        None
    } else {
        Some(query.clone())
    };
    let history_headers = Some(
        headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect::<HashMap<String, String>>(),
    );
    let history_body = if body.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&body).to_string())
    };
    let history_function = function.function_name.clone();

    // Plugin on_request hook: allow plugins to modify the event before invocation
    let event = if let Some(ref pm) = state.plugin_manager {
        if !pm.request_interceptors().is_empty() {
            match serde_json::to_value(&event) {
                Ok(event_json) => {
                    match pm
                        .on_request(method.as_ref(), &path, event_json, &function.function_name)
                        .await
                    {
                        Ok(modified) => serde_json::from_value(modified).unwrap_or(event),
                        Err(e) => {
                            tracing::warn!("⚠️ Plugin on_request error (continuing): {}", e);
                            event
                        }
                    }
                }
                Err(_) => event,
            }
        } else {
            event
        }
    } else {
        event
    };

    let invoke_result = executor.invoke_raw_event(event).await.and_then(|raw| {
        serde_json::from_value::<crate::runtime::LambdaResponse>(raw)
            .map_err(|e| anyhow::anyhow!("Failed to parse Lambda response: {}", e))
    });

    match invoke_result {
        Ok(response) => {
            let duration = request_start.elapsed();
            let status = StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::OK);
            let response_body = response.body.unwrap_or_default();
            let response_size = response_body.len();

            let status_icon = if status.is_success() {
                "✅"
            } else if status.is_redirection() {
                "↪️"
            } else if status.is_client_error() {
                "⚠️"
            } else {
                "❌"
            };

            tracing::info!(
                http.status = status.as_u16(),
                http.method = %method,
                http.path = %path,
                http.duration_ms = duration.as_millis() as u64,
                http.response_bytes = response_size,
                lambda.function = %function.function_name,
                "← {} {} {} {} [{}] → {}",
                status_icon,
                status.as_u16(),
                method,
                path,
                format_duration(duration),
                format_bytes(response_size)
            );

            // Emit TUI event
            #[cfg(feature = "tui")]
            emit_tui_event(crate::tui::ui::RequestEvent {
                timestamp: format_timestamp(),
                method: method.to_string(),
                path: path.clone(),
                status: status.as_u16(),
                duration_ms: duration.as_millis() as u64,
                function: function.function_name.clone(),
                response_bytes: response_size,
            });

            // Log slow requests
            if duration.as_millis() > 3000 {
                tracing::warn!(
                    "🐢 Slow request: {} {} took {}",
                    method,
                    path,
                    format_duration(duration)
                );
            }

            // Record to history
            if let Some(ref history) = state.history {
                let truncated_body = if response_body.len() > 10240 {
                    Some(format!("{}...[truncated]", &response_body[..10240]))
                } else {
                    Some(response_body.clone())
                };
                let entry = crate::history::HistoryEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: format_timestamp(),
                    method: history_method,
                    path: history_path,
                    query: history_query,
                    headers: history_headers,
                    body: history_body,
                    function: history_function,
                    status: status.as_u16(),
                    response_body: truncated_body,
                    duration_ms: duration.as_millis() as u64,
                    port: state.port,
                };
                history.record(entry).await;
            }

            let mut builder = axum::response::Response::builder().status(response.status_code);

            if let Some(headers) = response.headers {
                for (key, value) in headers {
                    builder = builder.header(key, value);
                }
            }

            builder.body(response_body.into()).unwrap_or_else(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to build response",
                )
                    .into_response()
            })
        }
        Err(e) => {
            let duration = request_start.elapsed();

            // Record error to history
            if let Some(ref history) = state.history {
                let entry = crate::history::HistoryEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: format_timestamp(),
                    method: history_method,
                    path: history_path,
                    query: history_query,
                    headers: history_headers,
                    body: history_body,
                    function: history_function,
                    status: 500,
                    response_body: Some(e.to_string()),
                    duration_ms: duration.as_millis() as u64,
                    port: state.port,
                };
                history.record(entry).await;
            }

            // Emit TUI event for errors
            #[cfg(feature = "tui")]
            emit_tui_event(crate::tui::ui::RequestEvent {
                timestamp: format_timestamp(),
                method: method.to_string(),
                path: path.clone(),
                status: 500,
                duration_ms: duration.as_millis() as u64,
                function: function.function_name.clone(),
                response_bytes: 0,
            });

            tracing::error!(
                "← ❌ 500 {} {} [{}] error: {}",
                method,
                path,
                format_duration(duration),
                e
            );
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_microseconds() {
        let d = std::time::Duration::from_micros(500);
        assert_eq!(format_duration(d), "500µs");
    }

    #[test]
    fn test_format_duration_milliseconds() {
        let d = std::time::Duration::from_millis(42);
        let s = format_duration(d);
        assert!(s.contains("ms"), "Expected ms, got: {}", s);
    }

    #[test]
    fn test_format_duration_seconds() {
        let d = std::time::Duration::from_secs(2);
        let s = format_duration(d);
        assert!(s.contains("s"), "Expected seconds, got: {}", s);
        assert!(s.starts_with("2.00"));
    }

    #[test]
    fn test_format_bytes_small() {
        assert_eq!(format_bytes(42), "42B");
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(1023), "1023B");
    }

    #[test]
    fn test_format_bytes_kilobytes() {
        let s = format_bytes(2048);
        assert!(s.contains("KB"), "Expected KB, got: {}", s);
    }

    #[test]
    fn test_format_bytes_megabytes() {
        let s = format_bytes(2 * 1024 * 1024);
        assert!(s.contains("MB"), "Expected MB, got: {}", s);
    }

    #[test]
    fn test_build_cors_layer_default() {
        // Should not panic with None config
        let _layer = build_cors_layer(None);
    }

    #[test]
    fn test_build_cors_layer_custom() {
        let config = CorsConfig {
            allow_origins: vec!["https://example.com".to_string()],
            allow_methods: vec!["GET".to_string(), "POST".to_string()],
            allow_headers: vec!["Authorization".to_string()],
            expose_headers: vec!["X-Custom".to_string()],
            allow_credentials: true,
            max_age: Some(3600),
        };
        let _layer = build_cors_layer(Some(&config));
    }

    #[test]
    fn test_build_cors_layer_wildcard() {
        let config = CorsConfig {
            allow_origins: vec!["*".to_string()],
            allow_methods: vec![],
            allow_headers: vec!["*".to_string()],
            expose_headers: vec![],
            allow_credentials: false,
            max_age: None,
        };
        let _layer = build_cors_layer(Some(&config));
    }
}

/// Resolve layer paths for a Lambda function.
/// Looks up each layer reference in the config and resolves to a directory path.
pub fn resolve_layer_paths(
    function: &LambdaConfig,
    layers: &[crate::config::LayerConfig],
    source_dir: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    function
        .layers
        .iter()
        .filter_map(|layer_ref| {
            // Find the layer config by resource name
            let layer = layers.iter().find(|l| l.resource_name == *layer_ref)?;

            // Resolve the layer source path
            if let Some(src) = &layer.source_path {
                let path = if src.is_absolute() {
                    src.clone()
                } else {
                    source_dir.join(src)
                };

                // If it's a zip file, check for extracted directory alongside it
                let resolved = if path.extension().is_some_and(|e| e == "zip") {
                    // Look for a directory with the same name minus .zip
                    let dir = path.with_extension("");
                    if dir.is_dir() {
                        dir
                    } else {
                        // Try using the zip path parent as the layer dir
                        path.parent().map(|p| p.to_path_buf()).unwrap_or(path)
                    }
                } else if path.is_dir() {
                    path
                } else {
                    // Could be a directory path
                    path
                };

                if resolved.exists() {
                    let resolved = resolved.canonicalize().unwrap_or(resolved);
                    tracing::info!(
                        "📦 Layer '{}' resolved to: {}",
                        layer.layer_name,
                        resolved.display()
                    );
                    Some(resolved)
                } else {
                    tracing::warn!(
                        "⚠️ Layer '{}' path not found: {}",
                        layer.layer_name,
                        resolved.display()
                    );
                    None
                }
            } else {
                tracing::warn!(
                    "⚠️ Layer '{}' has no source_path configured",
                    layer.layer_name
                );
                None
            }
        })
        .collect()
}
