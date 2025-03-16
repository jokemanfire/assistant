use crate::modeldeal::chat::DialogueModel;
use crate::modeldeal::chat_voice::VoiceModel;
use crate::{config::Config, modeldeal::voice_chat::SpeechModel};
use log::{debug, info, warn};
use protos::grpc::model::{
    model_service_server::{ModelService as GrpcModelService, ModelServiceServer},
    ChatMessage, SpeechRequest, SpeechResponse, StreamingRequest, StreamingResponse, TextRequest,
    TextResponse,
};
use protos::grpc::mserver::server_service_client::ServerServiceClient;
use protos::grpc::mserver::server_service_server::{ServerService, ServerServiceServer};
use protos::grpc::mserver::{
    Empty, ForwardTextRequest, ForwardTextResponse, ModelListResponse, ModelStatusRequest,
    ModelStatusResponse,
};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tonic::{transport::Server, Request, Response, Status};

// Constant for timeout when checking for available runners
const AVAILABLE_RUNNERS_TIMEOUT: u64 = 1;

pub struct GrpcService {
    server: Server,
    config: Config,
    uuid: String,
    stream_service: Option<Arc<crate::service::streamservice::StreamService>>,
}

// Server service implementation
struct ServerServiceImpl {
    config: Config,
    uuid: String,
}

// Model service implementation
pub struct ModelServiceImpl {
    chat_model: DialogueModel,
    voice_model: VoiceModel,
    speech_model: SpeechModel,
    local_service: Arc<crate::service::localservice::LocalService>,
    config: Config,
    stream_service: Option<Arc<crate::service::streamservice::StreamService>>,
}

#[tonic::async_trait]
impl GrpcModelService for ModelServiceImpl {
    // Speech to text conversion
    async fn speech_to_text(
        &self,
        _request: Request<SpeechRequest>,
    ) -> Result<Response<TextResponse>, Status> {
        // Currently just returns an empty response
        Ok(Response::new(TextResponse::default()))
    }

    // Text to speech conversion
    async fn text_to_speech(
        &self,
        _request: Request<TextRequest>,
    ) -> Result<Response<SpeechResponse>, Status> {
        // Currently just returns an empty response
        Ok(Response::new(SpeechResponse::default()))
    }

    // Process text chat and generate response
    async fn text_chat(
        &self,
        request: Request<TextRequest>,
    ) -> Result<Response<TextResponse>, Status> {
        let req = request.into_inner();
        info!("Received text chat request: {:?}", req.messages);

        // Try remote model first if configured
        if !self.chat_model.config.remote_models.is_empty() {
            match self
                .chat_model
                .get_response_online(req.messages.clone())
                .await
            {
                Ok(response) => {
                    return Ok(Response::new(TextResponse {
                        text: response,
                        ..Default::default()
                    }));
                }
                Err(e) => warn!("Remote model failed: {}", e),
            }
        }

        debug!("No remote model or remote failed, trying local model");
        // Try local model
        match timeout(
            Duration::from_secs(AVAILABLE_RUNNERS_TIMEOUT),
            // Get available runners in available_timeout
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
            Ok(_) => match self.local_service.chat(req.messages.clone()).await {
                Ok(response) => {
                    return Ok(Response::new(TextResponse {
                        text: response,
                        ..Default::default()
                    }));
                }
                Err(e) => warn!("Local model processing failed: {}", e),
            },
            Err(e) => warn!("No local model available: {}", e),
        }

        // If all attempts fail
        Err(Status::internal("All processing attempts failed"))
    }

    // Stream text chat and return WebSocket URL
    async fn streaming_text_chat(
        &self,
        request: Request<StreamingRequest>,
    ) -> Result<Response<StreamingResponse>, Status> {
        let req = request.into_inner();
        info!("Received streaming text chat request: {:?}", req.messages);

        // Check if stream service is available
        if let Some(stream_service) = &self.stream_service {
            // Generate WebSocket URL for streaming
            let session_id = req.session_id.clone();
            let ws_url = stream_service.generate_ws_url(req.messages, Some(session_id.clone()));

            // Return the streaming response with WebSocket URL
            return Ok(Response::new(StreamingResponse {
                streaming_url: ws_url,
                session_id,
            }));
        }

        // Stream service not available
        Err(Status::unavailable("Streaming service not available"))
    }
}

#[tonic::async_trait]
impl ServerService for ServerServiceImpl {
    async fn query_models(
        &self,
        _request: tonic::Request<Empty>,
    ) -> std::result::Result<tonic::Response<ModelListResponse>, tonic::Status> {
        use protos::grpc::mserver::ModelInfo;

        // Get models from both local and remote configurations
        let mut models = Vec::new();

        // Add local models
        for model in &self.config.chat_model.local_models {
            models.push(ModelInfo {
                model_id: model.model_path.clone(),
                name: model
                    .model_path
                    .split('/')
                    .last()
                    .unwrap_or("unknown")
                    .to_string(),
                description: format!("Local model with {} GPU layers", model.n_gpu_layers),
                status: if model.enabled { 2 } else { 0 }, // 2 = Ready, 0 = Unknown
            });
        }

        // Add remote models
        for model in &self.config.chat_model.remote_models {
            models.push(ModelInfo {
                model_id: model.model_name.clone(),
                name: model.model_name.clone(),
                description: format!("Remote model with priority {}", model.priority),
                status: if model.enabled { 2 } else { 0 }, // 2 = Ready, 0 = Unknown
            });
        }

        Ok(Response::new(ModelListResponse { models }))
    }

    /// Query model status
    async fn query_model_status(
        &self,
        request: tonic::Request<ModelStatusRequest>,
    ) -> std::result::Result<tonic::Response<ModelStatusResponse>, tonic::Status> {
        let request = request.into_inner();

        // Check local models first
        if let Some(model) = self
            .config
            .chat_model
            .local_models
            .iter()
            .find(|m| m.model_path == request.model_id)
        {
            return Ok(Response::new(ModelStatusResponse {
                status: if model.enabled { 2 } else { 0 }, // 2 = Ready, 0 = Unknown
                error: String::new(),
            }));
        }

        // Then check remote models
        if let Some(model) = self
            .config
            .chat_model
            .remote_models
            .iter()
            .find(|m| m.model_name == request.model_id)
        {
            return Ok(Response::new(ModelStatusResponse {
                status: if model.enabled { 2 } else { 0 }, // 2 = Ready, 0 = Unknown
                error: String::new(),
            }));
        }

        Err(Status::not_found(format!(
            "Model {} not found",
            request.model_id
        )))
    }

    /// Process text request, supports forwarding
    async fn process_text(
        &self,
        request: tonic::Request<ForwardTextRequest>,
    ) -> std::result::Result<tonic::Response<ForwardTextResponse>, tonic::Status> {
        let request = request.into_inner();

        // Check if we have a local model service to handle this
        if let Some(text_request) = &request.request {
            // Create a new model service instance
            let local_service = Arc::new(
                crate::service::localservice::LocalService::new(
                    self.config.chat_model.local_models.clone(),
                )
                .await,
            );

            let model_service = ModelServiceImpl {
                chat_model: DialogueModel {
                    config: self.config.chat_model.clone(),
                },
                voice_model: VoiceModel {
                    config: self.config.chat_voice.clone(),
                },
                speech_model: SpeechModel {
                    config: self.config.voice_chat.clone(),
                },
                local_service,
                config: self.config.clone(),
                stream_service: None,
            };

            // Process the request locally
            match model_service
                .text_chat(Request::new(text_request.clone()))
                .await
            {
                Ok(response) => {
                    return Ok(Response::new(ForwardTextResponse {
                        response: Some(response.into_inner()),
                        route_uuids: vec![self.uuid.clone()],
                        error: String::new(),
                    }));
                }
                Err(_) => {
                    // If local processing fails, try remote servers
                    return self.try_remote_servers(request).await;
                }
            }
        }

        // If no text request is provided
        Err(Status::invalid_argument("No text request provided"))
    }
}

impl ServerServiceImpl {
    async fn try_remote_servers(
        &self,
        request: ForwardTextRequest,
    ) -> Result<Response<ForwardTextResponse>, Status> {
        // Get remote server list
        let endpoints = &self.config.remote_server.endpoints;
        if endpoints.is_empty() {
            return Err(Status::unavailable("No remote endpoints configured"));
        }

        // Simple loop to try each endpoint
        for endpoint in endpoints {
            match self
                .try_forward_to_endpoint(endpoint, request.clone())
                .await
            {
                Ok(response) => return Ok(response),
                Err(_) => continue,
            }
        }

        Err(Status::unavailable("All remote endpoints failed"))
    }

    async fn try_forward_to_endpoint(
        &self,
        endpoint: &str,
        request: ForwardTextRequest,
    ) -> Result<Response<ForwardTextResponse>, Box<dyn Error>> {
        let mut request = request;
        // Create connection to remote server
        let channel = tonic::transport::Channel::from_shared(endpoint.to_string())?
            .timeout(std::time::Duration::from_millis(
                self.config.remote_server.timeout.unwrap_or(5000),
            ))
            .connect()
            .await?;

        // Create client
        let mut client = ServerServiceClient::new(channel);

        let mut parameters = request.parameters.clone();

        if let Some(try_max_time) = request.parameters.get("try_max_time") {
            parameters.insert("try_max_time".to_string(), try_max_time.clone());
        } else {
            parameters.insert(
                "try_max_time".to_string(),
                self.config.server.try_max_time.unwrap_or(3).to_string(),
            );
        }
        if !request.route_uuids.contains(&self.uuid) {
            request.route_uuids.push(self.uuid.clone());
        }
        let try_time = match request.parameters.get("try_time") {
            Some(try_time) => {
                //try_time +1
                (try_time.parse::<u32>().unwrap() + 1).to_string()
            }
            None => "1".to_string(),
        };
        //check try_time
        if try_time.parse::<u32>().unwrap()
            > parameters
                .get("try_max_time")
                .unwrap()
                .parse::<u32>()
                .unwrap()
        {
            return Err(Box::new(Status::unavailable("Try time out")));
        }
        request.parameters = parameters;

        let response = client.process_text(request).await?;
        Ok(response)
    }
}

impl GrpcService {
    pub async fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let server = Server::builder();
        let uuid = uuid::Uuid::new_v4().to_string();
        Ok(Self {
            server,
            config,
            uuid,
            stream_service: None,
        })
    }

    // Initialize stream service
    pub async fn init_stream_service(&mut self) -> Result<(), Box<dyn Error>> {
        let stream_service =
            Arc::new(crate::service::streamservice::StreamService::new(self.config.clone()).await?);

        // Start the WebSocket server in the background
        stream_service.start_in_background().await.unwrap();

        self.stream_service = Some(stream_service);
        Ok(())
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let addr = self
            .config
            .server
            .grpc_addr
            .as_ref()
            .ok_or("Missing gRPC address")?
            .parse()?;

        // Create server service implementation
        let server_service = ServerServiceImpl {
            config: self.config.clone(),
            uuid: self.uuid.clone(),
        };

        // Create model service implementation
        let local_service = Arc::new(
            crate::service::localservice::LocalService::new(
                self.config.chat_model.local_models.clone(),
            )
            .await,
        );

        let model_service = ModelServiceImpl {
            chat_model: DialogueModel {
                config: self.config.chat_model.clone(),
            },
            voice_model: VoiceModel {
                config: self.config.chat_voice.clone(),
            },
            speech_model: SpeechModel {
                config: self.config.voice_chat.clone(),
            },
            local_service,
            config: self.config.clone(),
            stream_service: self.stream_service.clone(),
        };

        info!("Starting gRPC server on {}", addr);
        self.server
            .add_service(ServerServiceServer::new(server_service))
            .add_service(ModelServiceServer::new(model_service))
            .serve(addr)
            .await?;
        Ok(())
    }

    pub async fn start_in_background(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let addr = self
            .config
            .server
            .grpc_addr
            .as_ref()
            .ok_or("Missing gRPC address")?
            .parse()?;

        // Create server service implementation
        let server_service = ServerServiceImpl {
            config: self.config.clone(),
            uuid: self.uuid.clone(),
        };

        // Create model service implementation
        let local_service = Arc::new(
            crate::service::localservice::LocalService::new(
                self.config.chat_model.local_models.clone(),
            )
            .await,
        );

        let model_service = ModelServiceImpl {
            chat_model: DialogueModel {
                config: self.config.chat_model.clone(),
            },
            voice_model: VoiceModel {
                config: self.config.chat_voice.clone(),
            },
            speech_model: SpeechModel {
                config: self.config.voice_chat.clone(),
            },
            local_service,
            config: self.config.clone(),
            stream_service: self.stream_service.clone(),
        };

        info!("Starting gRPC server on {}", addr);

        // 创建服务器但不等待它完成
        let server = Server::builder()
            .add_service(ServerServiceServer::new(server_service))
            .add_service(ModelServiceServer::new(model_service));

        // 在后台启动服务器
        tokio::spawn(async move {
            if let Err(e) = server.serve(addr).await {
                log::error!("gRPC server error: {}", e);
            }
        });

        // 等待一小段时间确保服务器已启动
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(())
    }
}
