mod api;
mod auth;
mod models;

use crate::config::OpenAIConfig;
use crate::http::{AppState, HttpPlugin};
use crate::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::{Json, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::post,
    Router,
};
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;

pub use self::api::{chat_completions, completions};
pub use self::auth::verify_api_key;
pub use self::models::{
    ChatCompletionRequest, ChatCompletionResponse, CompletionRequest, CompletionResponse,
};

#[cfg(feature = "http_api")]
use assistant_client::AssistantClient;

/// OpenAI plugin
pub struct OpenAIPlugin {
    name: String,
    config: OpenAIConfig,
    #[cfg(feature = "http_api")]
    model_service: Option<Arc<Mutex<AssistantClient>>>,
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
    pub async fn with_model_service(mut self, endpoint: String) -> Result<Self, anyhow::Error> {
        let model_service = AssistantClient::connect(endpoint.as_str()).await?;
        self.model_service = Some(Arc::new(Mutex::new(model_service)));
        Ok(self)
    }

    pub fn create_router(&self) -> Router {
        #[cfg(feature = "http_api")]
        {
            if let Some(model_service) = &self.model_service {
                let state = AppState::new(OpenAIState {
                    config: self.config.clone(),
                    model_service: model_service.clone(),
                });
                // use assistant client to create router
                return Router::new()
                    .route("/v1/chat/completions", post(chat_completions))
                    .route("/v1/completions", post(completions))
                    // Now we need to verify api key now
                    // .layer(middleware::from_fn_with_state(
                    //     state.clone(),
                    //     verify_api_key,
                    // ))
                    .with_state(state);
            }
        }

        // if http_api feature is not enabled, return a empty router
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
            info!("OpenAI plugin is disabled");
            return Ok(());
        }

        #[cfg(not(feature = "http_api"))]
        {
            warn!("http_api feature is not enabled, OpenAI compatible API will be disabled");
            return Ok(());
        }

        #[cfg(feature = "http_api")]
        {
            if self.model_service.is_none() {
                warn!("No model service provided, OpenAI compatible API will be disabled");
            }
        }

        Ok(())
    }

    async fn start(&self) -> Result<()> {
        // start by HttpPlugin
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        // stop by HttpPlugin
        Ok(())
    }
}

/// OpenAI API state
#[cfg(feature = "http_api")]
#[derive(Clone)]
pub struct OpenAIState {
    pub config: OpenAIConfig,
    pub model_service: Arc<Mutex<AssistantClient>>,
}
