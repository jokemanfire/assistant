mod api;
mod models;
mod auth;

use crate::config::OpenAIConfig;
use crate::error::PluginError;
use crate::http::{AppState, HttpPlugin};
use crate::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    Router,
    routing::post,
    extract::{State, Json},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
};
use log::{info, warn};
use std::sync::Arc;

pub use self::api::{chat_completions, completions};
pub use self::models::{ChatCompletionRequest, ChatCompletionResponse, CompletionRequest, CompletionResponse};
pub use self::auth::verify_api_key;

#[cfg(feature = "http_api")]
use assistant_service::service::grpcservice::ModelServiceImpl;

/// OpenAI兼容API插件
pub struct OpenAIPlugin {
    name: String,
    config: OpenAIConfig,
    #[cfg(feature = "http_api")]
    model_service: Option<Arc<ModelServiceImpl>>,
}

impl OpenAIPlugin {
    pub fn new(config: OpenAIConfig) -> Self {
        Self {
            name: "openai".to_string(),
            config,
            #[cfg(feature = "http_api")]
            model_service: None,
        }
    }
    
    #[cfg(feature = "http_api")]
    pub fn with_model_service(mut self, service: Arc<ModelServiceImpl>) -> Self {
        self.model_service = Some(service);
        self
    }
    
    pub fn create_router(&self) -> Router {
        #[cfg(feature = "http_api")]
        {
            if let Some(model_service) = &self.model_service {
                let state = AppState::new(OpenAIState {
                    config: self.config.clone(),
                    model_service: model_service.clone(),
                });
                
                return Router::new()
                    .route("/v1/chat/completions", post(chat_completions))
                    .route("/v1/completions", post(completions))
                    .layer(middleware::from_fn_with_state(state.clone(), verify_api_key))
                    .with_state(state);
            }
        }
        
        // 如果没有启用http_api特性或没有model_service，返回一个空路由
        Router::new()
    }
}

#[async_trait]
impl Plugin for OpenAIPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn init(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("OpenAI兼容API已禁用");
            return Ok(());
        }
        
        #[cfg(not(feature = "http_api"))]
        {
            warn!("未启用http_api特性，OpenAI兼容API将不可用");
            return Ok(());
        }
        
        #[cfg(feature = "http_api")]
        {
            if self.model_service.is_none() {
                warn!("未提供模型服务，OpenAI兼容API将不可用");
            }
        }
        
        Ok(())
    }
    
    async fn start(&self) -> Result<()> {
        // 启动由HttpPlugin处理
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        // 停止由HttpPlugin处理
        Ok(())
    }
}

/// OpenAI API状态
#[cfg(feature = "http_api")]
pub struct OpenAIState {
    pub config: OpenAIConfig,
    pub model_service: Arc<ModelServiceImpl>,
} 