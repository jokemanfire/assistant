use crate::error::PluginError;
use crate::http::AppState;
use crate::openai::models::{
    ChatCompletionChoice, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionUsage,
    ChatMessage, CompletionChoice, CompletionRequest, CompletionResponse, Role,
};
#[cfg(feature = "http_api")]
use crate::openai::OpenAIState;
use axum::{extract::State, Json};
use log::{debug, error, info};
use protos::grpc::model::{ChatMessage as GrpcChatMessage, Role as GrpcRole};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Deal with chat completion request
#[cfg(feature = "http_api")]
pub async fn chat_completions(
    State(state): State<AppState<OpenAIState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, PluginError> {
    info!("receive chat completion request, model: {}", request.model);
    debug!("request details: {:?}", request);

    // get state
    let state_guard = state.inner.lock().await;
    let mut model_service = state_guard.model_service.lock().await;
    let config = &state_guard.config;

    // map OpenAI model name to local model
    let model_name = config
        .model_mapping
        .get(&request.model)
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    // convert message format
    let grpc_messages: Vec<GrpcChatMessage> = request
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
            // create response
            let response = ChatCompletionResponse {
                id: format!("chatcmpl-{}", Uuid::new_v4()),
                object: "chat.completion".to_string(),
                created: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                model: request.model,
                choices: vec![ChatCompletionChoice {
                    index: 0,
                    message: ChatMessage {
                        role: Role::Assistant,
                        content: response_text.clone(),
                        name: None,
                    },
                    finish_reason: "stop".to_string(),
                }],
                usage: ChatCompletionUsage {
                    prompt_tokens: 0, // 我们不跟踪token使用情况
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("chat completion request failed: {}", e);
            Err(PluginError::ModelServiceError(e.to_string()))
        }
    }
}

/// 处理文本完成请求
#[cfg(feature = "http_api")]
pub async fn completions(
    State(state): State<AppState<OpenAIState>>,
    Json(request): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, PluginError> {
    info!("get completion request, model: {}", request.model);
    debug!("request details: {:?}", request);

    // get state
    let state_guard = state.inner.lock().await;
    let mut model_service = state_guard.model_service.lock().await;
    let config = &state_guard.config;

    // map OpenAI model name to local model
    let model_name = config
        .model_mapping
        .get(&request.model)
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    // Create grpc messages
    let grpc_messages = vec![GrpcChatMessage {
        role: GrpcRole::User as i32,
        content: request.prompt.clone(),
        ..Default::default()
    }];

    match model_service.chat(grpc_messages).await {
        Ok(response_text) => {
            // Create response
            let response = CompletionResponse {
                id: format!("cmpl-{}", Uuid::new_v4()),
                object: "text_completion".to_string(),
                created: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                model: request.model,
                choices: vec![CompletionChoice {
                    index: 0,
                    text: response_text.clone(),
                    finish_reason: "stop".to_string(),
                }],
                usage: ChatCompletionUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("text completion request failed: {}", e);
            Err(PluginError::ModelServiceError(e.to_string()))
        }
    }
}

#[cfg(not(feature = "http_api"))]
pub async fn chat_completions(
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, PluginError> {
    Err(PluginError::HttpError("未启用http_api特性".to_string()))
}

#[cfg(not(feature = "http_api"))]
pub async fn completions(
    Json(request): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, PluginError> {
    Err(PluginError::HttpError("未启用http_api特性".to_string()))
}
