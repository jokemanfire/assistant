use log::{debug, error, info};
use std::{env, error::Error, process::exit, sync::Arc};
use tokio::{signal::unix::signal, sync::Mutex};
pub mod config;
pub mod local;
pub mod modeldeal;
pub mod service;

pub struct MainServer {
    grpc_service: Arc<Mutex<service::grpcservice::GrpcService>>,
}

impl MainServer {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let config = config::Config::new();
        debug!("Creating grpc service");
        let grpc_service = Arc::new(Mutex::new(
            service::grpcservice::GrpcService::new(config.clone()).await?,
        ));

        Ok(Self { grpc_service })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // start all services
        self.grpc_service.lock().await.start().await?;
        let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt())?;
        info!("All servers started");
        interrupt.recv().await;
        // Graceful shutdown can be implemented here if needed
        Ok(())
    }
}

fn print_default_config() -> Result<(), Box<dyn Error>> {
    let default_config = include_str!("default.toml");
    println!("{}", default_config);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // init logger with debug level
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "config" {
        return print_default_config();
    }

    let mut server = MainServer::new().await?;
    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        exit(-1);
    }

    Ok(())
}
