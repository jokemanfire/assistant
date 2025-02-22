use log::{debug, error, info};
use std::{error::Error, process::exit, sync::Arc};
use tokio::{signal::unix::signal, sync::Mutex};
pub mod config;
pub mod local;
pub mod modeldeal;
pub mod service;

pub struct MainServer {
    ttrpc_service: Arc<Mutex<service::ttrpcservice::TtrpcService>>,
    grpc_service: Arc<Mutex<service::grpcservice::GrpcService>>,
}

impl MainServer {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let config = config::Config::new();
        debug!("Creating local service");
        let local_service = Arc::new(
            service::localservice::LocalService::new(config.dialogue_model.local_models.clone())
                .await,
        );
        debug!("Creating grpc service");
        let grpc_service = Arc::new(Mutex::new(service::grpcservice::GrpcService::new(config.clone()).await?));
        debug!("Creating ttrpc service");
        let ttrpc_service = Arc::new(Mutex::new(
            service::ttrpcservice::TtrpcService::new(config.clone(), local_service.clone()).await?,
        ));

        Ok(Self {
            ttrpc_service,
            grpc_service,
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // start all services
        self.ttrpc_service.lock().await.start().await?;
        self.grpc_service.lock().await.start().await?;
        let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt())?;
        info!("All servers started");
        interrupt.recv().await;
        self.ttrpc_service.lock().await.shutdown().await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // init logger with debug level
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));

    let mut server = MainServer::new().await?;
    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        exit(-1);
    }

    Ok(())
}
