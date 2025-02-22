use crate::config::{Config, LocalModelConfig, ModelConfig};
use crate::modeldeal::dialogue_model::DialogueModel;
use async_trait::async_trait;
use log::{debug, info, warn};
use protos::ttrpc::{model, model_ttrpc};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tonic::{transport::Channel, Request};
use ttrpc::r#async::Server;

const AVAILABLE_RUNNERS_TIMEOUT: u64 = 1;

pub struct TtrpcService {
    server: Server,
    config: Config,
}

struct ModelS {
    chat_model: DialogueModel,
    local_service: Arc<crate::service::localservice::LocalService>,
    config: Config,
}

impl TtrpcService {
    pub async fn new(
        config: Config,
        local_service: Arc<crate::service::localservice::LocalService>,
    ) -> Result<Self, Box<dyn Error>> {
        let addr = config
            .server
            .ttrpc_addr
            .as_ref()
            .ok_or("Missing ttrpc address")?;
        remove_if_sock_exist(addr.as_str())?;

        let model_service = model_ttrpc::create_model_service(Arc::new(ModelS {
            chat_model: DialogueModel {
                config: config.dialogue_model.clone(),
            },
            local_service: local_service.clone(),
            config: config.clone(),
        }));

        info!("Starting ttrpc server on {}", addr);
        let server = Server::new()
            .bind(addr.as_str())?
            .register_service(model_service);

        Ok(Self { server, config })
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Starting ttrpc server");
        self.server.start().await?;
        Ok(())
    }
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.server.shutdown().await?;
        Ok(())
    }
}

impl ModelS {
    async fn forward_to_grpc(
        &self,
        request: protos::grpc::model::TextRequest,
    ) -> Result<model::TextResponse, Box<dyn Error>> {
        let grpc_addr = self
            .config
            .remote_server
            .endpoints
            .first()
            .ok_or("gRPC address not configured")?;

        let channel = Channel::from_shared(grpc_addr.clone())?
            .connect()
            .await?;

        let mut client =
            protos::grpc::mserver::server_service_client::ServerServiceClient::new(channel);

        let response = client
            .process_text(Request::new(protos::grpc::mserver::ForwardTextRequest {
                request: Some(protos::grpc::model::TextRequest {
                    text: request.text,
                    ..Default::default()
                }),
                ..Default::default()
            }))
            .await?;
        Ok(protos::ttrpc::model::TextResponse {
            text: response.into_inner().response.unwrap().text,
            ..Default::default()
        })
    }
}

#[async_trait]
pub trait ModelDeal<S, R> {
    async fn get_response_online(&self, inputdata: S) -> Result<R, Box<dyn std::error::Error>>;
}

#[async_trait]
impl model_ttrpc::ModelService for ModelS {
    async fn text_chat(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        req: model::TextRequest,
    ) -> ::ttrpc::Result<model::TextResponse> {
        info!("Received text chat request: {:?}", req.text);
        // try remote model if remote model fail, try local model
        if !self.chat_model.config.remote_models.is_empty() {
            match self.chat_model.get_response_online(req.text.clone()).await {
                Ok(response) => {
                    return Ok(model::TextResponse {
                        text: response,
                        ..Default::default()
                    });
                }
                Err(e) => warn!("Remote model failed: {}", e),
            }
        }
        debug!("No remote model, trying local model");
        // try local model
        match timeout(
            Duration::from_secs(AVAILABLE_RUNNERS_TIMEOUT as u64),
            // get available runners in available_timeout
            {
                let local_service = self.local_service.clone();
                tokio::spawn(async move {
                    loop {
                        let available_runners = local_service.available_runners().await;
                        if available_runners > 0 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
            },
        )
        .await
        {
            Ok(_) => {
                let response = self.local_service.chat(req.text.clone()).await.unwrap();
                return Ok(model::TextResponse {
                    text: response,
                    ..Default::default()
                });
            }
            Err(e) => warn!("No local model available: {}", e),
        }

        // if local model is busy, try to forward through gRPC
        if let Some(grpc_addr) = &self.config.server.grpc_addr {
            info!(
                "Attempting to forward request through gRPC to {}",
                grpc_addr
            );
            match self
                .forward_to_grpc(protos::grpc::model::TextRequest {
                    text: req.text,
                    ..Default::default()
                })
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) => warn!("gRPC forward failed: {}", e),
            }
        }

        Err(ttrpc::Error::Others(
            "All processing attempts failed".into(),
        ))
    }

    async fn speech_to_text(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        _req: model::SpeechRequest,
    ) -> ::ttrpc::Result<model::TextResponse> {
        let res = model::TextResponse::default();
        Ok(res)
    }
    async fn text_to_speech(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        _req: model::TextRequest,
    ) -> ::ttrpc::Result<model::SpeechResponse> {
        let res = model::SpeechResponse::default();
        Ok(res)
    }
}

pub fn remove_if_sock_exist(sock_addr: &str) -> Result<(), Box<dyn Error>> {
    let path = sock_addr
        .strip_prefix("unix://")
        .expect("socket address is not expected");

    if std::path::Path::new(path).exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_ttrpc_server() {
        let config = Config::new();
        let local_service = Arc::new(crate::service::localservice::LocalService::new(config.dialogue_model.local_models.clone()).await);
        let mut ttrpc_service = TtrpcService::new(config, local_service).await.unwrap();
        ttrpc_service.start().await.unwrap();
    }
}
