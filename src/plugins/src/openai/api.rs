use crate::error::PluginError;
use crate::http::AppState;
use crate::openai::models::{
    ChatCompletionChoice, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionUsage,
    ChatMessage, CompletionChoice, CompletionRequest, CompletionResponse, Role,
};
#[cfg(feature = "http_api")]
use crate::openai::OpenAIState;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use log::{debug, error, info};
use protos::grpc::model::{ChatMessage as GrpcChatMessage, Role as GrpcRole};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Deal with chat completion request
#[cfg(feature = "http_api")]
pub async fn chat_completions(
    State(state): State<AppState<OpenAIState>>,
    request: axum::http::Request<axum::body::Body>,
) -> Result<Response, PluginError> {
    // Get Content-Type header
    let headers = request.headers().clone();
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or("unknown"))
        .unwrap_or("not provided");

    info!(
        "Received chat completion request, Content-Type: {}",
        content_type
    );

    // Parse request body
    let bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return Err(PluginError::HttpError(
                "Failed to read request body".to_string(),
            ));
        }
    };

    // Parse request based on Content-Type
    let chat_request: ChatCompletionRequest = match content_type {
        ct if ct.contains("application/json") => match serde_json::from_slice(&bytes) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse JSON request: {}", e);
                return Err(PluginError::HttpError(
                    "Failed to parse JSON request".to_string(),
                ));
            }
        },
        // Add support for other formats here if needed
        _ => {
            error!("Unsupported Content-Type: {}", content_type);
            return Err(PluginError::HttpError(format!(
                "Unsupported Content-Type: {}",
                content_type
            )));
        }
    };

    info!(
        "Received chat completion request, model: {}",
        chat_request.model
    );
    debug!("Request details: {:?}", chat_request);

    // get state
    let state_guard = state.inner.lock().await;
    let mut model_service = state_guard.model_service.lock().await;

    // convert message format
    let grpc_messages: Vec<GrpcChatMessage> = chat_request
        .messages
        .iter()
        .map(|msg| GrpcChatMessage {
            role: match msg.role {
                Role::System => GrpcRole::System as i32,
                Role::User => GrpcRole::User as i32,
                Role::Assistant => GrpcRole::Assistant as i32,
                _ => GrpcRole::User as i32, // default to user
            },
            content: msg.content.clone(),
            ..Default::default()
        })
        .collect();

    // call gRPC service
    match model_service.chat(grpc_messages).await {
        Ok(response_text) => {
            // Check if streaming is requested
            if chat_request.stream.unwrap_or(false) {
                // Create a streaming response
                let id = Uuid::new_v4().to_string();
                let created = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // Create the first chunk with role
                let first_chunk = json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": chat_request.model,
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "role": "assistant"
                        },
                        "finish_reason": null
                    }]
                });

                // Create the content chunk
                let content_chunk = json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": chat_request.model,
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "content": response_text
                        },
                        "finish_reason": "stop"
                    }]
                });

                // Create the [DONE] message
                let done_message = "[DONE]";

                // Combine all chunks into a single response with proper SSE format
                let body = format!(
                    "data: {}\n\ndata: {}\n\ndata: {}\n\n",
                    first_chunk.to_string(),
                    content_chunk.to_string(),
                    done_message
                );

                // Create response with appropriate headers
                let response = axum::response::Response::builder()
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .body(axum::body::Body::from(body))
                    .unwrap();

                return Ok(response);
            }

            // Non-streaming response (original code)
            let response = ChatCompletionResponse {
                id: format!("{}", Uuid::new_v4()),
                object: "chat.completion".to_string(),
                created: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                model: chat_request.model,
                choices: vec![ChatCompletionChoice {
                    index: 0,
                    message: ChatMessage {
                        role: Role::Assistant,
                        content: response_text.clone(),
                        name: None,
                    },
                    logprobs: None,
                    finish_reason: "stop".to_string(),
                }],
                usage: ChatCompletionUsage {
                    prompt_tokens: 23,
                    completion_tokens: 10,
                    total_tokens: 33,
                    completion_tokens_details: None,
                    system_fingerprint: None,
                },
            };
            let r = Json(response);
            debug!("Response: {:?}", r);
            Ok(r.into_response())
        }
        Err(e) => {
            error!("Chat completion request failed: {}", e);
            Err(PluginError::ModelServiceError(e.to_string()))
        }
    }
}

/// deal with completion request
#[cfg(feature = "http_api")]
pub async fn completions(
    State(state): State<AppState<OpenAIState>>,
    request: axum::http::Request<axum::body::Body>,
) -> Result<Response, PluginError> {
    // Get Content-Type header
    let headers = request.headers().clone();
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or("unknown"))
        .unwrap_or("not provided");

    info!(
        "Received completion request, Content-Type: {}",
        content_type
    );

    // Parse request body
    let bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return Err(PluginError::HttpError(
                "Failed to read request body".to_string(),
            ));
        }
    };

    // Parse request based on Content-Type
    let completion_request: CompletionRequest = match content_type {
        ct if ct.contains("application/json") => match serde_json::from_slice(&bytes) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse JSON request: {}", e);
                return Err(PluginError::HttpError(
                    "Failed to parse JSON request".to_string(),
                ));
            }
        },
        // Add support for other formats here if needed
        _ => {
            error!("Unsupported Content-Type: {}", content_type);
            return Err(PluginError::HttpError(format!(
                "Unsupported Content-Type: {}",
                content_type
            )));
        }
    };

    info!(
        "Received completion request, model: {}",
        completion_request.model
    );
    debug!("Request details: {:?}", completion_request);

    // get state
    let state_guard = state.inner.lock().await;
    let mut model_service = state_guard.model_service.lock().await;

    // Create grpc messages
    let grpc_messages = vec![GrpcChatMessage {
        role: GrpcRole::User as i32,
        content: completion_request.prompt.clone(),
        ..Default::default()
    }];

    match model_service.chat(grpc_messages).await {
        Ok(response_text) => {
            // Check if streaming is requested
            if completion_request.stream.unwrap_or(false) {
                // Create a streaming response
                let id = Uuid::new_v4().to_string();
                let created = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // Create the content chunk
                let content_chunk = json!({
                    "id": id,
                    "object": "text_completion.chunk",
                    "created": created,
                    "model": completion_request.model,
                    "choices": [{
                        "index": 0,
                        "text": response_text,
                        "finish_reason": "stop"
                    }]
                });

                // Create the [DONE] message
                let done_message = "[DONE]";

                // Combine all chunks into a single response with proper SSE format
                let body = format!(
                    "data: {}\n\ndata: {}\n\n",
                    content_chunk.to_string(),
                    done_message
                );

                // Create response with appropriate headers
                let response = axum::response::Response::builder()
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .body(axum::body::Body::from(body))
                    .unwrap();

                return Ok(response);
            }

            // Create response
            let response = CompletionResponse {
                id: format!("{}", Uuid::new_v4()),
                object: "text_completion".to_string(),
                created: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                model: completion_request.model,
                choices: vec![CompletionChoice {
                    index: 0,
                    text: response_text.clone(),
                    finish_reason: "stop".to_string(),
                }],
                usage: ChatCompletionUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    completion_tokens_details: None,
                    system_fingerprint: None,
                },
            };

            Ok(Json(response).into_response())
        }
        Err(e) => {
            error!("Text completion request failed: {}", e);
            Err(PluginError::ModelServiceError(e.to_string()))
        }
    }
}

#[cfg(not(feature = "http_api"))]
pub async fn chat_completions(
    request: axum::http::Request<axum::body::Body>,
) -> Result<Response, PluginError> {
    Err(PluginError::HttpError(
        "http_api feature is not enabled".to_string(),
    ))
}

#[cfg(not(feature = "http_api"))]
pub async fn completions(
    request: axum::http::Request<axum::body::Body>,
) -> Result<Response, PluginError> {
    Err(PluginError::HttpError(
        "http_api feature is not enabled".to_string(),
    ))
}
