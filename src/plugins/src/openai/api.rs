use crate::error::PluginError;
use crate::http::AppState;
use crate::openai::{
    models::{
        ChatCompletionChoice, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionUsage,
        ChatMessage, CompletionChoice, CompletionRequest, CompletionResponse, Role,
    },
    OpenAIState,
};
use axum::{extract::State, Json};
use log::{debug, error, info};
use protos::grpc::model::{ChatMessage as GrpcChatMessage, Role as GrpcRole};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 处理聊天完成请求
#[cfg(feature = "http_api")]
pub async fn chat_completions(
    State(state): State<AppState<OpenAIState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, PluginError> {
    info!("收到聊天完成请求，模型: {}", request.model);
    debug!("请求详情: {:?}", request);
    
    // 获取状态
    let state_guard = state.inner.lock().await;
    let model_service = &state_guard.model_service;
    let config = &state_guard.config;
    
    // 将OpenAI模型名映射到本地模型
    let model_name = config
        .model_mapping
        .get(&request.model)
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    
    // 转换消息格式
    let grpc_messages: Vec<GrpcChatMessage> = request
        .messages
        .iter()
        .map(|msg| GrpcChatMessage {
            role: match msg.role {
                Role::System => GrpcRole::System as i32,
                Role::User => GrpcRole::User as i32,
                Role::Assistant => GrpcRole::Assistant as i32,
                _ => GrpcRole::User as i32, // 默认为用户
            },
            content: msg.content.clone(),
            ..Default::default()
        })
        .collect();
    
    // 调用gRPC服务
    match model_service.text_chat_internal(grpc_messages).await {
        Ok(response_text) => {
            // 创建响应
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
            error!("聊天完成请求失败: {}", e);
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
    info!("收到文本完成请求，模型: {}", request.model);
    debug!("请求详情: {:?}", request);
    
    // 获取状态
    let state_guard = state.inner.lock().await;
    let model_service = &state_guard.model_service;
    let config = &state_guard.config;
    
    // 将OpenAI模型名映射到本地模型
    let model_name = config
        .model_mapping
        .get(&request.model)
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    
    // 创建消息
    let grpc_messages = vec![GrpcChatMessage {
        role: GrpcRole::User as i32,
        content: request.prompt.clone(),
        ..Default::default()
    }];
    
    // 调用gRPC服务
    match model_service.text_chat_internal(grpc_messages).await {
        Ok(response_text) => {
            // 创建响应
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
            error!("文本完成请求失败: {}", e);
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