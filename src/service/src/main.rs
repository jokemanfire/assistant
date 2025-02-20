use log::{error, info};
use std::{error::Error, process::exit, sync::Arc};
use tokio::signal::unix::signal;
pub mod config;
pub mod local;
pub mod modeldeal;
pub mod service;

pub struct MainServer {
    config: config::Config,
    local_service: Arc<service::localservice::LocalService>,
    ttrpc_service: Arc<service::ttrpcservice::TtrpcService>,
    grpc_service: Arc<service::grpcservice::GrpcService>,
}

impl MainServer {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let config = config::Config::new();

        let local_service = Arc::new(
            service::localservice::LocalService::new(config.dialogue_model.local_models.clone())
                .await,
        );
        let grpc_service = Arc::new(service::grpcservice::GrpcService::new(config.clone()).await?);
        let ttrpc_service = Arc::new(
            service::ttrpcservice::TtrpcService::new(config.clone(), local_service.clone()).await?,
        );

        Ok(Self {
            config,
            local_service,
            ttrpc_service,
            grpc_service,
        })
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        // 启动所有服务
        if let Some(ttrpc_service) = Arc::get_mut(&mut self.ttrpc_service.clone()) {
            ttrpc_service.start().await?;
        }
        if let Some(grpc_service) = Arc::get_mut(&mut self.grpc_service.clone()) {
            grpc_service.start().await?;
        }

        let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt())?;
        info!("All servers started");
        interrupt.recv().await;

        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let server = MainServer::new().await?;
    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        exit(-1);
    }

    Ok(())
}
