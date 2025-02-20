use crate::config::Config;
use log::{info, warn};
use protos::grpc::model::model_service_server::ModelServiceServer;
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
}

struct ModelService {
    config: Config,
}

#[tonic::async_trait]
impl ServerService for ModelService {
    async fn query_models(
        &self,
        request: tonic::Request<Empty>,
    ) -> std::result::Result<tonic::Response<ModelListResponse>, tonic::Status> {
        todo!()
    }
    /// 查询特定模型状态
    async fn query_model_status(
        &self,
        request: tonic::Request<ModelStatusRequest>,
    ) -> std::result::Result<tonic::Response<ModelStatusResponse>, tonic::Status> {
        todo!()
    }
    /// 处理文本请求，支持转发
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
        // 获取远程服务器列表
        let endpoints = &self.config.remote_server.endpoints;
        if endpoints.is_empty() {
            return Err(Status::unavailable("No remote endpoints configured"));
        }

        // 简单的循环尝试每个端点
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
        // 创建到远程服务器的连接
        let channel = tonic::transport::Channel::from_shared(endpoint.to_string())?
            .timeout(std::time::Duration::from_millis(
                self.config.remote_server.timeout.unwrap_or(5000),
            ))
            .connect()
            .await.unwrap();

        // 创建客户端
        let mut client = ServerServiceClient::new(channel);

        // 转发请求
        let response = client.process_text(Request::new(request)).await.unwrap();
        Ok(response)
    }
}

impl GrpcService {
    pub async fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let server = Server::builder();
        Ok(Self { server, config })
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
        };

        info!("Starting gRPC server on {}", addr);
        self.server
            .add_service(ServerServiceServer::new(model_service))
            .serve(addr)
            .await?;
        Ok(())
    }
}
