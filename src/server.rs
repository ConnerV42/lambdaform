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

use crate::config::LambdaformConfig;
use crate::router::Router as LambdaRouter;
use crate::runtime::{FunctionExecutor, LambdaEvent};

/// Shared application state
pub struct AppState {
    pub router: LambdaRouter,
    pub config: LambdaformConfig,
    pub source_dir: std::path::PathBuf,
}

/// Start the HTTP server
pub async fn start_server(
    config: LambdaformConfig,
    source_dir: std::path::PathBuf,
    port: u16,
) -> anyhow::Result<()> {
    let router = LambdaRouter::new(&config.gateways, &config.functions);
    
    let state = Arc::new(AppState {
        router,
        config,
        source_dir,
    });
    
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
    
    // Match route
    let matched = match state.router.match_request(&http_method, &path) {
        Some(m) => m,
        None => {
            return (StatusCode::NOT_FOUND, "Route not found").into_response();
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
        query_string_parameters: if query.is_empty() { None } else { Some(query) },
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
    
    // Execute function
    let executor = FunctionExecutor::new(
        matched.function.clone(),
        state.source_dir.clone(),
    );
    
    match executor.invoke(event).await {
        Ok(response) => {
            let mut builder = axum::response::Response::builder()
                .status(response.status_code);
            
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
