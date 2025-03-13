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
            grpc_service,
            #[cfg(feature = "http_api")]
            plugin_manager,
        })
    }

    #[cfg(feature = "http_api")]
    pub async fn init_plugins(&mut self, model_service: Arc<service::grpcservice::ModelServiceImpl>) -> Result<(), Box<dyn Error>> {
        // 创建插件配置
        let plugins_config = PluginsConfig::default();
        
        // 创建插件管理器
        let mut manager = PluginManager::new();
        
        // 创建OpenAI插件
        let openai_plugin = OpenAIPlugin::new(plugins_config.openai.clone())
            .with_model_service(model_service);
        
        // 创建HTTP插件
        let http_plugin = HttpPlugin::new(plugins_config.http.clone())
            .with_router(openai_plugin.create_router());
        
        // 注册插件
        manager.register(openai_plugin);
        manager.register(http_plugin);
        
        // 初始化所有插件
        manager.init_all().await?;
        
        self.plugin_manager = Some(manager);
        
        Ok(())
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // 获取模型服务实例
        #[cfg(feature = "http_api")]
        let model_service = {
            let service = self.grpc_service.lock().await.get_model_service().await?;
            self.init_plugins(service).await?;
            
            // 启动所有插件
            if let Some(manager) = &self.plugin_manager {
                manager.start_all().await?;
            }
        };
        
        // 启动所有服务
        self.grpc_service.lock().await.start().await?;
        let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt())?;
        info!("All servers started");
        interrupt.recv().await;
        
        // 停止所有插件
        #[cfg(feature = "http_api")]
        if let Some(manager) = &self.plugin_manager {
            manager.stop_all().await?;
        }
        
        // 优雅关闭可以在这里实现
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
