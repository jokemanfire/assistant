use protos::assistant::{
    assistant_service_server::{AssistantService, AssistantServiceServer},
    InfoRequest, InfoResponse, Request, Response,
};
use scheduler::Scheduler;
use std::sync::Arc;
use tonic::{transport::Server, Request as TonicRequest, Response as TonicResponse, Status};
use tracing::{debug, info, warn};
use tokio_stream::wrappers::ReceiverStream;

pub struct GrpcServer {
    scheduler: Arc<Scheduler>,
    max_load: f32,
    remote_servers: Vec<RemoteServerConfig>,
}

#[derive(Clone)]
pub struct RemoteServerConfig {
    pub name: String,
    pub grpc_addr: String,
    pub weight: u32,
    pub enabled: bool,
}

impl GrpcServer {
    pub fn new(scheduler: Arc<Scheduler>, max_load: f32, remote_servers: Vec<RemoteServerConfig>) -> Self {
        Self { 
            scheduler,
            max_load,
            remote_servers,
        }
    }

    pub async fn serve(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let addr = addr.parse()?;
        info!("Starting gRPC server on {}", addr);

        Server::builder()
            .add_service(AssistantServiceServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    // try to forward request to remote servers
    async fn try_remote_forward(&self, request: Request) -> Result<Response, Status> {
        for server in &self.remote_servers {
            if !server.enabled {
                continue;
            }

            match self.forward_to_remote(&server.grpc_addr, request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => warn!("Failed to forward to {}: {}", server.name, e),
            }
        }
        
        Err(Status::resource_exhausted("All servers are busy"))
    }

    // forward request to specified remote server
    async fn forward_to_remote(&self, addr: &str, request: Request) -> Result<Response, Status> {
        let mut client = protos::assistant::assistant_service_client::AssistantServiceClient::connect(
            format!("http://{}", addr)
        ).await.map_err(|e| Status::unavailable(e.to_string()))?;

        client.forward_request(request)
            .await
            .map(|r| r.into_inner())
    }

}

#[tonic::async_trait]
impl AssistantService for GrpcServer {
    type ForwardRequestStreamStream = ReceiverStream<Result<Response, Status>>;

    async fn forward_request(
        &self,
        request: TonicRequest<Request>,
    ) -> Result<TonicResponse<Response>, Status> {
        let request = request.into_inner();

        // check if local scheduler is busy
        if self.scheduler.is_busy(self.max_load).await {
            debug!("Local scheduler is busy, trying remote servers");
            return Ok(TonicResponse::new(self.try_remote_forward(request).await?));
        }

        // forward request to local scheduler
        match self.scheduler.forward_request(
            &request.path,
            &request.method,
            request.body,
            request.headers,
        ).await {
            Ok((status, body, headers)) => Ok(TonicResponse::new(Response {
                status: status as i32,
                body,
                headers,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn forward_request_stream(
        &self,
        request: TonicRequest<Request>,
    ) -> Result<TonicResponse<Self::ForwardRequestStreamStream>, Status> {
        let request = request.into_inner();
        
        // check if local scheduler is busy
        if self.scheduler.is_busy(self.max_load).await {
            debug!("Local scheduler is busy, trying remote servers");
            return Err(Status::resource_exhausted("All servers are busy"));
        }

        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let scheduler = self.scheduler.clone();
        let path = request.path.clone();
        let method = request.method.clone();
        let body = request.body.clone();
        let headers = request.headers.clone();

        // start background task to handle stream request
        tokio::spawn(async move {
            match scheduler.forward_request(
                &path,
                &method,
                body,
                headers,
            ).await {
                Ok((status, body, headers)) => {
                    let _ = tx.send(Ok(Response {
                        status: status as i32,
                        body,
                        headers,
                    })).await;
                }
                Err(e) => {
                    let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                }
            }
        });

        Ok(TonicResponse::new(ReceiverStream::new(rx)))
    }

    async fn get_info(
        &self,
        _request: TonicRequest<InfoRequest>,
    ) -> Result<TonicResponse<InfoResponse>, Status> {
        let instances = self.scheduler.list_instances().await;
        // get all model info
        let models = instances
            .iter()
            .map(|i| i.config.chat_model_path.clone().unwrap_or("".to_string()))
            .collect();

        let endpoints = vec![
            "/v1/chat/completions".to_string(),
            "/v1/completions".to_string(),
            "/v1/models".to_string(),
            "/v1/embeddings".to_string(),
            "/v1/chunks".to_string(),
            "/v1/audio/speech".to_string(),
            "/v1/info".to_string(),
        ];

        Ok(TonicResponse::new(InfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            models,
            endpoints,
        }))
    }
} 