use crate::config::HttpConfig;
use crate::error::PluginError;
use crate::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    Router,
    routing::{get, post},
    http::Method,
    extract::State,
};
use log::{info, error};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;

/// HTTP服务插件
pub struct HttpPlugin {
    name: String,
    config: HttpConfig,
    router: Option<Router>,
    shutdown_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

impl HttpPlugin {
    pub fn new(config: HttpConfig) -> Self {
        Self {
            name: "http".to_string(),
            config,
            router: None,
            shutdown_signal: None,
        }
    }
    
    pub fn with_router(mut self, router: Router) -> Self {
        self.router = Some(router);
        self
    }
    
    async fn health_check() -> &'static str {
        "OK"
    }
}

#[async_trait]
impl Plugin for HttpPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn init(&mut self) -> Result<()> {
        if self.router.is_none() {
            // 创建默认路由
            let router = Router::new()
                .route("/health", get(Self::health_check));
            self.router = Some(router);
        }
        
        Ok(())
    }
    
    async fn start(&self) -> Result<()> {
        if let Some(router) = &self.router {
            let mut final_router = router.clone();
            
            // 添加CORS支持
            if self.config.enable_cors {
                let cors = CorsLayer::new()
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers(Any)
                    .allow_origin(Any);
                final_router = final_router.layer(cors);
            }
            
            // 添加日志
            if self.config.enable_logging {
                final_router = final_router.layer(TraceLayer::new_for_http());
            }
            
            // 创建地址
            let addr = SocketAddr::new(
                self.config.host.parse().unwrap_or_else(|_| "127.0.0.1".parse().unwrap()),
                self.config.port,
            );
            
            // 创建关闭信号
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            let mut shutdown_signal = Some(tx);
            std::mem::swap(&mut shutdown_signal, &mut self.shutdown_signal.clone());
            
            // 启动服务器
            info!("HTTP服务启动在 {}", addr);
            tokio::spawn(async move {
                match axum::Server::bind(&addr)
                    .serve(final_router.into_make_service())
                    .with_graceful_shutdown(async {
                        rx.await.ok();
                    })
                    .await
                {
                    Ok(_) => info!("HTTP服务已停止"),
                    Err(e) => error!("HTTP服务错误: {}", e),
                }
            });
            
            Ok(())
        } else {
            Err(PluginError::StartError("路由未初始化".to_string()).into())
        }
    }
    
    async fn stop(&self) -> Result<()> {
        if let Some(signal) = &self.shutdown_signal {
            let _ = signal.send(());
            info!("已发送HTTP服务停止信号");
            Ok(())
        } else {
            Err(PluginError::StopError("HTTP服务未启动".to_string()).into())
        }
    }
}

/// 创建共享状态
pub struct AppState<T> {
    pub inner: Arc<Mutex<T>>,
}

impl<T> AppState<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }
}

/// 从AppState中提取内部值
pub async fn with_state<T, F, R>(state: State<AppState<T>>, f: F) -> Result<R, PluginError>
where
    F: FnOnce(&mut T) -> Result<R, PluginError>,
{
    let mut guard = state.inner.lock().await;
    f(&mut *guard)
} 