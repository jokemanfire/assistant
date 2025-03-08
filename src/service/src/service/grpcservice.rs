use crate::config::Config;
use log::{info, warn};
use protos::grpc::model::{TextRequest, TextResponse};
use protos::grpc::mserver::server_service_client::ServerServiceClient;
use protos::grpc::mserver::server_service_server::{ServerService, ServerServiceServer};
use protos::grpc::mserver::{
    Empty, ForwardTextRequest, ForwardTextResponse, ModelListResponse, ModelStatusRequest,
    ModelStatusResponse,
};
use std::error::Error;
use tonic::{transport::Server, Request, Response, Status};
use ttrpc::r#async::Client as TtrpcClient;

pub struct GrpcService {
    server: Server,
    config: Config,
    uuid: String,
}

struct ModelService {
    config: Config,
    uuid: String,
}

#[tonic::async_trait]
impl ServerService for ModelService {
    async fn query_models(
        &self,
        _request: tonic::Request<Empty>,
    ) -> std::result::Result<tonic::Response<ModelListResponse>, tonic::Status> {
        use protos::grpc::mserver::ModelInfo;
        
        // Get models from both local and remote configurations
        let mut models = Vec::new();
        
        // Add local models
        for model in &self.config.dialogue_model.local_models {
            models.push(ModelInfo {
                model_id: model.model_path.clone(),
                name: model.model_path.split('/').last().unwrap_or("unknown").to_string(),
                description: format!("Local model with {} GPU layers", model.n_gpu_layers),
                status: if model.enabled { 2 } else { 0 }, // 2 = Ready, 0 = Unknown
            });
        }

        // Add remote models
        for model in &self.config.dialogue_model.remote_models {
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
        if let Some(model) = self.config.dialogue_model.local_models
            .iter()
            .find(|m| m.model_path == request.model_id) {
            return Ok(Response::new(ModelStatusResponse {
                status: if model.enabled { 2 } else { 0 }, // 2 = Ready, 0 = Unknown
                error: String::new(),
            }));
        }

        // Then check remote models
        if let Some(model) = self.config.dialogue_model.remote_models
            .iter()
            .find(|m| m.model_name == request.model_id) {
            return Ok(Response::new(ModelStatusResponse {
                status: if model.enabled { 2 } else { 0 }, // 2 = Ready, 0 = Unknown
                error: String::new(),
            }));
        }

        Err(Status::not_found(format!("Model {} not found", request.model_id)))
    }

    /// Process text request, supports forwarding
    async fn process_text(
        &self,
        request: tonic::Request<ForwardTextRequest>,
    ) -> std::result::Result<tonic::Response<ForwardTextResponse>, tonic::Status> {
        let request = request.into_inner();
        // 创建 ttrpc client 并转发请求
        let ttrpc_addr = self
            .config
            .server
            .ttrpc_addr
            .as_ref()
            .ok_or_else(|| Status::internal("TTRPC address not configured"))?;

        let client = TtrpcClient::connect(ttrpc_addr)
            .map_err(|e| Status::internal(format!("Failed to connect to TTRPC server: {}", e)))?;

        let ttrpc_req = protos::ttrpc::model::TextRequest {
            text: request.clone().request.unwrap().text,
            ..Default::default()
        };

        let ttrpc_svc = protos::ttrpc::model_ttrpc::ModelServiceClient::new(client);
        match ttrpc_svc
            .text_chat(ttrpc::context::Context::default(), &ttrpc_req)
            .await
        {
            Ok(response) => Ok(Response::new(ForwardTextResponse {
                response: Some(TextResponse {
                    text: response.text,
                    ..Default::default()
                }),
                route_uuids: vec![],
                ..Default::default()
            })),
            Err(e) => {
                warn!("TTRPC forward failed: {}", e);
                // 如果 TTRPC 失败，尝试转发到远程服务器
                self.try_remote_servers(request).await
            }
        }
    }
}

impl ModelService {
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
            .await.unwrap();

        // Create client
        let mut client = ServerServiceClient::new(channel);

        let mut parameters = request.parameters.clone();
  
        if let Some(try_max_time) = request.parameters.get("try_max_time") {
            parameters.insert("try_max_time".to_string(), try_max_time.clone());
        } else {
            parameters.insert("try_max_time".to_string(), self.config.server.try_max_time.unwrap_or(3).to_string());
        }
        if !request.route_uuids.contains(&self.uuid) {
            request.route_uuids.push(self.uuid.clone());
        }
        let try_time = match request.parameters.get("try_time") {
            Some(try_time) => {
                //try_time +1
                (try_time.parse::<u32>().unwrap() + 1).to_string()
            },
            None => "1".to_string(),
        };
        //check try_time
        if try_time.parse::<u32>().unwrap() >  parameters.get("try_max_time").unwrap().parse::<u32>().unwrap() {
            return Err(Box::new(Status::unavailable("Try time out")));
        }
        request.parameters = parameters;

        let response = client.process_text(request).await.unwrap();
        Ok(response)
    }
}

impl GrpcService {
    pub async fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let server = Server::builder();
        let uuid = uuid::Uuid::new_v4().to_string();
        Ok(Self { server, config, uuid})
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        let addr = self
            .config
            .server
            .grpc_addr
            .as_ref()
            .ok_or("Missing gRPC address")?
            .parse()?;

        let model_service = ModelService {
            config: self.config.clone(),
            uuid: self.uuid.clone(),
        };

        info!("Starting gRPC server on {}", addr);
        self.server
            .add_service(ServerServiceServer::new(model_service))
            .serve(addr)
            .await?;
        Ok(())
    }

}
