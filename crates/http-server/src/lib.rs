use axum::{
    extract::{Path, Request},
    http::{HeaderMap, StatusCode, header::{HeaderName, HeaderValue}},
    response::{Response, IntoResponse},
    routing::{get, post},
    Router, body::Body,
};
use tokio::net::TcpListener;
use http_body_util::BodyExt;
use protos::assistant::assistant_service_client::AssistantServiceClient;
use tower_http::cors::CorsLayer;
use tracing::{debug, info};
use std::sync::Arc;
use serde_json;
use tokio_stream::wrappers::ReceiverStream;

pub struct HttpServer {
    grpc_addr: String,
}

impl HttpServer {
    pub fn new(grpc_addr: String) -> Self {
        Self { grpc_addr }
    }

    pub async fn serve(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let grpc_addr = Arc::new(self.grpc_addr);
        
        let app = Router::new()
            .route("/v1/chat/completions", post({
                let grpc_addr = Arc::clone(&grpc_addr);
                move |req| handle_request(req, (*grpc_addr).clone())
            }))
            .route("/v1/completions", post({
                let grpc_addr = Arc::clone(&grpc_addr);
                move |req| handle_request(req, (*grpc_addr).clone())
            }))
            .route("/v1/models", get({
                let grpc_addr = Arc::clone(&grpc_addr);
                move |req| handle_request(req, (*grpc_addr).clone())
            }))
            .route("/v1/embeddings", post({
                let grpc_addr = Arc::clone(&grpc_addr);
                move |req| handle_request(req, (*grpc_addr).clone())
            }))
            .route("/v1/chunks", post({
                let grpc_addr = Arc::clone(&grpc_addr);
                move |req| handle_request(req, (*grpc_addr).clone())
            }))
            .route("/v1/audio/speech", post({
                let grpc_addr = Arc::clone(&grpc_addr);
                move |req| handle_request(req, (*grpc_addr).clone())
            }))
            .route("/v1/info", get({
                let grpc_addr = Arc::clone(&grpc_addr);
                move |req| handle_request(req, (*grpc_addr).clone())
            }))
            .layer(CorsLayer::permissive());

        let listener = TcpListener::bind(addr).await?;
        info!("Starting HTTP server on {}", addr);

        axum::serve(listener, app.into_make_service())
            .await?;

        Ok(())
    }
}

async fn handle_request(
    req: Request<Body>,
    grpc_addr: String,
) -> Result<Response<Body>, StatusCode> {
    debug!("Received request to path: {}", req.uri().path());

    // Extract request components
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();
    let headers = req.headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = req.into_body().collect().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .to_bytes()
        .to_vec();

    // Check if this is a stream request
    let is_stream = if let Ok(body_str) = String::from_utf8(body.clone()) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_str) {
            json.get("stream").and_then(|v| v.as_bool()).unwrap_or(false)
        } else {
            false
        }
    } else {
        false
    };

    // Create gRPC request
    let request = protos::assistant::Request {
        path,
        method,
        headers,
        body,
    };

    // Forward to gRPC server
    let mut client = AssistantServiceClient::connect("http://".to_string() + &grpc_addr)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if is_stream {
        let stream = client
            .forward_request_stream(request)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .into_inner();

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(4);
        let mut stream = stream;

        // Start background task to handle stream
        tokio::spawn(async move {
            while let Some(chunk) = stream.message().await.map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "stream error")).unwrap() {
                let _ = tx.send(Ok(chunk.body)).await;
            }
        });

        // Create streaming response
        let stream = ReceiverStream::new(rx);
        let body = Body::from_stream(stream);
        
        let mut builder = Response::builder()
            .status(200)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive");

        builder
            .body(body)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    } else {
        let response = client
            .forward_request(request)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .into_inner();

        // Convert back to HTTP response
        let mut builder = Response::builder()
            .status(response.status as u16);

        // Add headers
        let headers = builder.headers_mut().unwrap();
        for (key, value) in response.headers {
            if let Ok(name) = HeaderName::from_bytes(key.as_bytes()) {
                if let Ok(val) = HeaderValue::from_str(&value) {
                    headers.insert(name, val);
                }
            }
        }

        builder
            .body(Body::from(response.body))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
} 