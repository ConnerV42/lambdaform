use lambda_http::{run, service_fn, Body, Error, Request, Response};
use serde_json::json;

async fn handler(event: Request) -> Result<Response<Body>, Error> {
    let method = event.method().as_str();
    let path = event.uri().path();

    let (status, body) = match method {
        "GET" => {
            if path == "/" || path.is_empty() {
                (200, json!({
                    "message": "Hello from Rust Lambda!",
                    "runtime": "provided.al2023",
                    "language": "Rust"
                }))
            } else {
                let id = path.trim_start_matches('/');
                (200, json!({
                    "id": id,
                    "language": "Rust",
                    "found": true
                }))
            }
        }
        "POST" => {
            let body_str = std::str::from_utf8(event.body().as_ref()).unwrap_or("{}");
            let mut parsed: serde_json::Value = serde_json::from_str(body_str).unwrap_or(json!({}));
            if let Some(obj) = parsed.as_object_mut() {
                obj.insert("id".to_string(), json!("rust-456"));
                obj.insert("created".to_string(), json!(true));
            }
            (201, parsed)
        }
        _ => (405, json!({"error": format!("Method {} not allowed", method)})),
    };

    let resp = Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body)?))
        .map_err(Box::new)?;

    Ok(resp)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}
