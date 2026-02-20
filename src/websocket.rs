//! WebSocket API Gateway emulation
//!
//! Provides a WebSocket server that routes messages to Lambda functions
//! based on route selection expressions, mimicking AWS API Gateway WebSocket APIs.

use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;

use crate::config::{LambdaConfig, LambdaformConfig};
use crate::pool::ProcessPool;
use crate::runtime::{
    DebugOptions, FunctionExecutor, RequestIdentity, WebSocketEvent, WebSocketRequestContext,
};

/// Shared state for WebSocket connections
pub struct WsState {
    pub config: RwLock<LambdaformConfig>,
    pub source_dir: std::path::PathBuf,
    pub debug: Option<DebugOptions>,
    pub pool: Arc<ProcessPool>,
    /// Route key → function resource name mapping
    pub routes: RwLock<HashMap<String, WsRoute>>,
    /// Active connections: connection_id → sender
    pub connections: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Message>>>,
    /// Route selection expression (e.g., "$request.body.action")
    pub route_selection_expression: String,
    /// Gateway resource name
    #[allow(dead_code)]
    pub gateway_resource: String,
}

#[derive(Debug, Clone)]
pub struct WsRoute {
    #[allow(dead_code)]
    pub route_key: String,
    pub function_resource: String,
}

impl WsState {
    pub fn new(
        config: LambdaformConfig,
        gateway_resource: &str,
        source_dir: std::path::PathBuf,
        route_selection_expression: String,
        debug: Option<DebugOptions>,
    ) -> Self {
        let routes = Self::build_routes(&config, gateway_resource);
        Self {
            config: RwLock::new(config),
            source_dir,
            debug,
            pool: Arc::new(ProcessPool::new()),
            routes: RwLock::new(routes),
            connections: Mutex::new(HashMap::new()),
            route_selection_expression,
            gateway_resource: gateway_resource.to_string(),
        }
    }

    fn build_routes(config: &LambdaformConfig, gateway_resource: &str) -> HashMap<String, WsRoute> {
        let mut routes = HashMap::new();
        if let Some(gw) = config
            .gateways
            .iter()
            .find(|g| g.resource_name == gateway_resource)
        {
            for route in &gw.routes {
                // For WebSocket, the path IS the route key ($connect, $disconnect, $default, or custom)
                let route_key = route.path.clone();
                routes.insert(
                    route_key.clone(),
                    WsRoute {
                        route_key,
                        function_resource: route.function_resource.clone(),
                    },
                );
            }
        }
        routes
    }

    /// Find function config by resource name
    async fn find_function(&self, resource_name: &str) -> Option<LambdaConfig> {
        let config = self.config.read().await;
        config
            .functions
            .iter()
            .find(|f| f.resource_name == resource_name)
            .cloned()
    }

    /// Resolve the route key from a message body using the route selection expression
    fn resolve_route_key(&self, body: &str) -> Option<String> {
        // Parse the route selection expression
        // Common format: "$request.body.action" → extract "action" field from JSON body
        let expr = &self.route_selection_expression;

        if let Some(field) = expr.strip_prefix("$request.body.") {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) {
                return parsed
                    .get(field)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }

        None
    }

    /// Send a message to a connection (used by @connections API emulation)
    pub async fn post_to_connection(&self, connection_id: &str, data: &str) -> bool {
        let connections = self.connections.lock().await;
        if let Some(tx) = connections.get(connection_id) {
            tx.send(Message::Text(data.to_string())).is_ok()
        } else {
            false
        }
    }
}

/// Start a WebSocket server for a gateway
pub async fn start_websocket_server(
    config: LambdaformConfig,
    gateway_resource: &str,
    source_dir: std::path::PathBuf,
    port: u16,
    route_selection_expression: String,
    debug: Option<DebugOptions>,
) -> anyhow::Result<()> {
    let state = Arc::new(WsState::new(
        config,
        gateway_resource,
        source_dir,
        route_selection_expression,
        debug,
    ));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("🔌 WebSocket server listening on ws://{}", addr);

    // Start a small HTTP server for @connections management API on port+1
    let connections_state = state.clone();
    if let Some(connections_port) = port.checked_add(1) {
        tokio::spawn(start_connections_api(connections_state, connections_port));
    } else {
        tracing::warn!(
            "⚠️ Cannot start @connections API: port {} + 1 overflows u16. \
             Use a lower WebSocket port to enable the management API.",
            port
        );
    }

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        let state = state.clone();
                        tokio::spawn(handle_connection(state, stream, peer_addr));
                    }
                    Err(e) => {
                        tracing::error!("WebSocket accept error: {}", e);
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("🛑 WebSocket server shutting down...");
                break;
            }
        }
    }

    Ok(())
}

/// Handle a single WebSocket connection
async fn handle_connection(state: Arc<WsState>, stream: TcpStream, peer_addr: SocketAddr) {
    let connection_id = format!(
        "{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );

    let connected_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    tracing::info!(
        "🔌 New WebSocket connection from {} (id: {})",
        peer_addr,
        connection_id
    );

    // Upgrade to WebSocket
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::error!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Create channel for sending messages back to this connection
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    // Register connection
    {
        let mut connections = state.connections.lock().await;
        connections.insert(connection_id.clone(), tx);
    }

    // Invoke $connect handler
    let connect_result = invoke_route_handler(
        &state,
        "$connect",
        "CONNECT",
        &connection_id,
        connected_at,
        None,
        Some(&peer_addr),
    )
    .await;

    if let Some(response) = connect_result {
        if response.status_code >= 400 {
            tracing::warn!(
                "← ⚠️ $connect handler rejected connection {} (status {})",
                connection_id,
                response.status_code
            );
            let mut connections = state.connections.lock().await;
            connections.remove(&connection_id);
            return;
        }
    }

    // Spawn task to forward messages from channel to WebSocket
    let send_connection_id = connection_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                tracing::debug!("WebSocket send failed for {}", send_connection_id);
                break;
            }
        }
    });

    // Process incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                tracing::info!("→ WS {} message: {} bytes", connection_id, text.len());

                // Resolve route from message body
                let route_key = state
                    .resolve_route_key(&text)
                    .unwrap_or_else(|| "$default".to_string());

                let response = invoke_route_handler(
                    &state,
                    &route_key,
                    "MESSAGE",
                    &connection_id,
                    connected_at,
                    Some(&text),
                    None,
                )
                .await;

                // If handler returns a body, send it back
                if let Some(resp) = response {
                    if let Some(body) = resp.body {
                        if !body.is_empty() {
                            let connections = state.connections.lock().await;
                            if let Some(tx) = connections.get(&connection_id) {
                                let _ = tx.send(Message::Text(body));
                            }
                        }
                    }
                }
            }
            Ok(Message::Binary(data)) => {
                tracing::info!("→ WS {} binary: {} bytes", connection_id, data.len());
                // Binary messages go to $default
                let response = invoke_route_handler(
                    &state,
                    "$default",
                    "MESSAGE",
                    &connection_id,
                    connected_at,
                    Some(&base64_encode(&data)),
                    None,
                )
                .await;

                if let Some(resp) = response {
                    if let Some(body) = resp.body {
                        if !body.is_empty() {
                            let connections = state.connections.lock().await;
                            if let Some(tx) = connections.get(&connection_id) {
                                let _ = tx.send(Message::Text(body));
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                tracing::info!("🔌 WebSocket {} closing", connection_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                let connections = state.connections.lock().await;
                if let Some(tx) = connections.get(&connection_id) {
                    let _ = tx.send(Message::Pong(data));
                }
            }
            Ok(_) => {} // Pong, Frame
            Err(e) => {
                tracing::error!("WebSocket error for {}: {}", connection_id, e);
                break;
            }
        }
    }

    // Invoke $disconnect handler
    let _ = invoke_route_handler(
        &state,
        "$disconnect",
        "DISCONNECT",
        &connection_id,
        connected_at,
        None,
        None,
    )
    .await;

    // Clean up connection
    {
        let mut connections = state.connections.lock().await;
        connections.remove(&connection_id);
    }

    send_task.abort();
    tracing::info!("🔌 WebSocket {} disconnected", connection_id);
}

/// Invoke a Lambda handler for a WebSocket route
async fn invoke_route_handler(
    state: &WsState,
    route_key: &str,
    event_type: &str,
    connection_id: &str,
    connected_at: u64,
    body: Option<&str>,
    peer_addr: Option<&SocketAddr>,
) -> Option<crate::runtime::LambdaResponse> {
    let routes = state.routes.read().await;

    // Try exact match first, then fall back to $default
    let route = routes.get(route_key).or_else(|| {
        if route_key != "$connect" && route_key != "$disconnect" {
            routes.get("$default")
        } else {
            None
        }
    });

    let route = match route {
        Some(r) => r.clone(),
        None => {
            tracing::debug!("No handler for WebSocket route: {}", route_key);
            return None;
        }
    };
    drop(routes);

    let function = match state.find_function(&route.function_resource).await {
        Some(f) => f,
        None => {
            tracing::error!(
                "Function '{}' not found for route '{}'",
                route.function_resource,
                route_key
            );
            return None;
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let event = WebSocketEvent {
        request_context: WebSocketRequestContext {
            route_key: route_key.to_string(),
            event_type: event_type.to_string(),
            connection_id: connection_id.to_string(),
            stage: "local".to_string(),
            api_id: "lambdaform".to_string(),
            request_id: format!("ws-{:x}", now.as_nanos()),
            domain_name: "localhost".to_string(),
            request_time_epoch: now.as_millis() as u64,
            message_id: if event_type == "MESSAGE" {
                Some(format!("msg-{:x}", now.as_nanos()))
            } else {
                None
            },
            identity: RequestIdentity {
                source_ip: peer_addr
                    .map(|a| a.ip().to_string())
                    .unwrap_or_else(|| "127.0.0.1".to_string()),
            },
            connected_at: Some(connected_at),
        },
        body: body.map(|s| s.to_string()),
        is_base64_encoded: false,
        headers: if event_type == "CONNECT" {
            Some(HashMap::new())
        } else {
            None
        },
        multi_value_headers: None,
        query_string_parameters: None,
    };

    let event_json = match serde_json::to_value(&event) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to serialize WebSocket event: {}", e);
            return None;
        }
    };

    // Resolve layer paths
    let config = state.config.read().await;
    let layer_paths =
        crate::server::resolve_layer_paths(&function, &config.layers, &state.source_dir);
    let archive_files = config.archive_files.clone();
    drop(config);

    let fn_source_dir =
        function.resolve_source_dir_with_archives(&state.source_dir, &archive_files);
    let executor = FunctionExecutor::new(function.clone(), fn_source_dir)
        .with_debug(state.debug.clone())
        .with_pool(Some(state.pool.clone()))
        .with_layer_paths(layer_paths);

    let start = std::time::Instant::now();

    // Send the proper WebSocketEvent to the Lambda function (not LambdaEvent)
    match executor.invoke_raw_event(event_json).await {
        Ok(raw_result) => {
            let duration = start.elapsed();
            // Parse response — WebSocket handlers return { statusCode, body } like HTTP handlers
            let status_code = raw_result
                .get("statusCode")
                .and_then(|v| v.as_u64())
                .unwrap_or(200) as u16;
            let body = raw_result
                .get("body")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let headers = raw_result
                .get("headers")
                .and_then(|v| serde_json::from_value::<HashMap<String, String>>(v.clone()).ok());
            let is_base64_encoded = raw_result
                .get("isBase64Encoded")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            tracing::info!(
                "← WS {} {} → {} [{:.1}ms]",
                route_key,
                event_type,
                status_code,
                duration.as_secs_f64() * 1000.0
            );

            Some(crate::runtime::LambdaResponse {
                status_code,
                headers,
                body,
                is_base64_encoded,
            })
        }
        Err(e) => {
            let duration = start.elapsed();
            tracing::error!(
                "← ❌ WS {} {} error [{:.1}ms]: {}",
                route_key,
                event_type,
                duration.as_secs_f64() * 1000.0,
                e
            );
            None
        }
    }
}

/// Start a small HTTP API for @connections management (POST to connection)
async fn start_connections_api(state: Arc<WsState>, port: u16) {
    use axum::{
        body::Bytes,
        extract::{Path, State},
        http::StatusCode,
        routing::post,
        Router,
    };

    let app = Router::new()
        .route(
            "/@connections/:connection_id",
            post(
                |State(state): State<Arc<WsState>>,
                 Path(connection_id): Path<String>,
                 body: Bytes| async move {
                    let data = String::from_utf8_lossy(&body).to_string();
                    if state.post_to_connection(&connection_id, &data).await {
                        StatusCode::OK
                    } else {
                        StatusCode::GONE
                    }
                },
            ),
        )
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            tracing::info!("📡 @connections API on http://localhost:{}", port);
            let _ = axum::serve(listener, app).await;
        }
        Err(e) => {
            tracing::warn!(
                "⚠️ Failed to bind @connections API on port {}: {}. \
                 WebSocket server will work but @connections POST API won't be available.",
                port,
                e
            );
        }
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_route_key_from_body() {
        let state = WsState {
            config: RwLock::new(crate::config::LambdaformConfig::default()),
            source_dir: std::path::PathBuf::from("."),
            debug: None,
            pool: Arc::new(ProcessPool::new()),
            routes: RwLock::new(HashMap::new()),
            connections: Mutex::new(HashMap::new()),
            route_selection_expression: "$request.body.action".to_string(),
            gateway_resource: "test".to_string(),
        };

        assert_eq!(
            state.resolve_route_key(r#"{"action": "sendmessage", "data": "hello"}"#),
            Some("sendmessage".to_string())
        );
    }

    #[test]
    fn test_resolve_route_key_missing_field() {
        let state = WsState {
            config: RwLock::new(crate::config::LambdaformConfig::default()),
            source_dir: std::path::PathBuf::from("."),
            debug: None,
            pool: Arc::new(ProcessPool::new()),
            routes: RwLock::new(HashMap::new()),
            connections: Mutex::new(HashMap::new()),
            route_selection_expression: "$request.body.action".to_string(),
            gateway_resource: "test".to_string(),
        };

        assert_eq!(state.resolve_route_key(r#"{"data": "hello"}"#), None);
    }

    #[test]
    fn test_resolve_route_key_invalid_json() {
        let state = WsState {
            config: RwLock::new(crate::config::LambdaformConfig::default()),
            source_dir: std::path::PathBuf::from("."),
            debug: None,
            pool: Arc::new(ProcessPool::new()),
            routes: RwLock::new(HashMap::new()),
            connections: Mutex::new(HashMap::new()),
            route_selection_expression: "$request.body.action".to_string(),
            gateway_resource: "test".to_string(),
        };

        assert_eq!(state.resolve_route_key("not json"), None);
    }
}
