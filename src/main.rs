use anyhow::Result;
use grpc_server::GrpcServer;
use http_server::HttpServer;
use scheduler::Scheduler;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn, Level};
use clap::{Parser, ArgAction};
use config::{Config, generate_example_config};
use tracing_subscriber::{filter::LevelFilter, prelude::*};

const DEFAULT_MODEL_CONFIG: &str = include_str!("../default.toml");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Print default model configuration and exit
    #[arg(long = "model-config", action = ArgAction::SetTrue)]
    print_model_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // If --model-config is specified, print config and exit
    if cli.print_model_config {
        println!("{}", DEFAULT_MODEL_CONFIG);
        return Ok(());
    }

    // Initialize logging
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("h2", LevelFilter::OFF)
        .with_target("hyper", LevelFilter::OFF)
        .with_target("tower", LevelFilter::OFF)
        .with_default(LevelFilter::DEBUG);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    info!("Starting assistant service");
    
    // Load configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            warn!("Failed to load config: {}, using default", e);
            generate_example_config()?;
            Config::default()
        }
    };

    // Create scheduler
    let scheduler = Arc::new(Scheduler::new(
        config.scheduler.config_dir.clone(),
        config.scheduler.max_instances,
    ));

    // Create config directory if it doesn't exist
    std::fs::create_dir_all(&config.scheduler.config_dir)?;

    // Load model instances from config directory
    if let Err(e) = scheduler.load_instances(config.llama_servers).await {
        warn!("Failed to load model instances: {}", e);
    }

    // Start gRPC server
    let remote_servers = config.remote_servers.into_iter().map(|cfg| grpc_server::RemoteServerConfig {
        name: cfg.name,
        grpc_addr: cfg.grpc_addr,
        weight: cfg.weight,
        enabled: cfg.enabled,
    }).collect();
    let grpc_server = GrpcServer::new(scheduler.clone(), config.scheduler.max_load, remote_servers);
    let grpc_addr = config.server.grpc_addr.clone();
    
    let grpc_handle = tokio::spawn(async move {
        info!("Starting gRPC server on {}", grpc_addr);
        if let Err(e) = grpc_server.serve(&grpc_addr).await {
            warn!("gRPC server error: {}", e);
        }
    });

    // Start HTTP server (if enabled)
    let http_handle = if let Some(http_addr) = config.server.http_addr {
        let http_server = HttpServer::new(config.server.grpc_addr.clone());
        
        Some(tokio::spawn(async move {
            info!("Starting HTTP server on {}", http_addr);
            if let Err(e) = http_server.serve(&http_addr).await {
                warn!("HTTP server error: {}", e);
            }
        }))
    } else {
        None
    };

    info!("Service startup completed");

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => info!("Received shutdown signal"),
        Err(e) => warn!("Failed to listen for shutdown signal: {}", e),
    }

    // Graceful shutdown
    info!("Starting graceful shutdown");

    // Stop all model instances
    let instances = scheduler.list_instances().await;
    for instance in instances {
        if let Err(e) = scheduler.stop_instance(&instance.id).await {
            warn!("Failed to stop instance {}: {}", instance.id, e);
        }
    }

    // Cancel server tasks
    if let Some(http_handle) = http_handle {
        http_handle.abort();
    }
    grpc_handle.abort();

    info!("Shutdown completed");
    Ok(())
} 