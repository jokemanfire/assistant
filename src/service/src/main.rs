use log::{debug, error, info};
use std::{env, error::Error, process::exit, sync::Arc};
use tokio::{signal::unix::signal, sync::Mutex};
pub mod config;
pub mod local;
pub mod modeldeal;
pub mod service;

#[cfg(feature = "http_api")]
use assistant_plugins::{
    config::{HttpConfig, OpenAIConfig, PluginsConfig},
    http::HttpPlugin,
    openai::OpenAIPlugin,
    PluginManager,
};

pub struct MainServer {
    config: config::Config,
    grpc_service: Arc<Mutex<service::grpcservice::GrpcService>>,
    #[cfg(feature = "http_api")]
    plugin_manager: Option<PluginManager>,
}

impl MainServer {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let config = config::Config::new();
        debug!("Creating grpc service");
        let grpc_service = Arc::new(Mutex::new(
            service::grpcservice::GrpcService::new(config.clone()).await?,
        ));

        #[cfg(feature = "http_api")]
        let plugin_manager = None;

        Ok(Self {
            config,
            grpc_service,
            #[cfg(feature = "http_api")]
            plugin_manager,
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Starting gRPC service...");

        let service = self.grpc_service.lock().await;
        service.start_in_background().await.unwrap();
        drop(service);

        info!("gRPC service started");

        // Initialize plugins
        #[cfg(feature = "http_api")]
        {
            info!("Initializing plugins...");
            // Create plugin configuration
            let plugins_config = PluginsConfig::default();

            // Create plugin manager
            let mut manager = PluginManager::new();

            // Create OpenAI plugin, connect to the already started gRPC service
            let grpc_addr = self
                .config
                .server
                .grpc_addr
                .clone()
                .unwrap_or("http://127.0.0.1:50051".to_string());
            let openai_plugin = OpenAIPlugin::new(plugins_config.openai.clone())
                .with_model_service(format!("http://{}", grpc_addr))
                .await?;
            info!("OpenAI plugin initialized");
            // Create HTTP plugin
            let host_port = self
                .config
                .server
                .http_addr
                .clone()
                .unwrap_or("127.0.0.1:8080".to_string());
            let host = host_port.split(":").next().unwrap_or("127.0.0.1");
            let port = host_port.split(":").nth(1).unwrap_or("8080");

            let http_plugin = HttpPlugin::new(HttpConfig {
                host: host.to_string(),
                port: port.parse().unwrap(),
                enable_cors: true,
                enable_logging: true,
            })
            .with_router(openai_plugin.create_router());

            // Register plugins
            manager.register(openai_plugin);
            manager.register(http_plugin);

            // Initialize all plugins
            manager.init_all().await?;

            self.plugin_manager = Some(manager);

            // Start all plugins
            if let Some(manager) = &self.plugin_manager {
                manager.start_all().await?;
            }

            info!("Plugins initialized and started");
        }

        // Wait for an interrupt signal
        let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt())?;
        info!("All servers started, press Ctrl+C to stop");
        interrupt.recv().await;

        // Stop all plugins
        #[cfg(feature = "http_api")]
        if let Some(manager) = &self.plugin_manager {
            info!("Stopping plugins...");
            manager.stop_all().await?;
        }

        info!("Graceful shutdown complete");
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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .filter(Some("actix_server"), log::LevelFilter::Off)
        .filter(Some("actix_web"), log::LevelFilter::Off)
        .filter(Some("h2"), log::LevelFilter::Off)
        .init();

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
