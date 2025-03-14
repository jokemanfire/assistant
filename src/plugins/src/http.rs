use crate::config::HttpConfig;
use crate::error::PluginError;
use crate::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::State,
    http::Method,
    routing::{get, post},
    serve::Serve,
    Router,
};
use log::{error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// HTTP service plugin
#[derive(Clone)]
pub struct HttpPlugin {
    name: String,
    config: HttpConfig,
    router: Option<Router>,
    close_signal: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl HttpPlugin {
    pub fn new(config: HttpConfig) -> Self {
        Self {
            name: "http".to_string(),
            config,
            router: None,
            close_signal: Arc::new(Mutex::new(None)),
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
            // create default router
            let router = Router::new().route("/health", get(Self::health_check));
            self.router = Some(router);
        }

        Ok(())
    }

    async fn start(&self) -> Result<()> {
        if let Some(router) = &self.router {
            let mut final_router = router.clone();

            // Add CORS support
            if self.config.enable_cors {
                let cors = CorsLayer::new()
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers(Any)
                    .allow_origin(Any);
                final_router = final_router.layer(cors);
            }

            // Add logging
            if self.config.enable_logging {
                final_router = final_router.layer(TraceLayer::new_for_http());
            }

            // Create address
            let addr = SocketAddr::new(
                self.config
                    .host
                    .parse()
                    .unwrap_or_else(|_| "127.0.0.1".parse().unwrap()),
                self.config.port,
            );

            // Create shutdown signal
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();

            // Start server
            info!("http service started at {}", addr);
            tokio::spawn(async move {
                let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
                match axum::serve(listener, final_router)
                    .with_graceful_shutdown(async {
                        rx.await.ok();
                    })
                    .await
                {
                    Ok(_) => info!("http service stopped"),
                    Err(e) => error!("http service error: {}", e),
                }
            });

            self.close_signal.lock().await.replace(tx);

            Ok(())
        } else {
            Err(PluginError::StartError("router not initialized".to_string()).into())
        }
    }

    async fn stop(&self) -> Result<()> {
        // Since we no longer save shutdown_signal, here we only log
        info!("HTTP service will stop after all connections are closed");
        let _ = self.close_signal.lock().await.take().unwrap().send(());
        Ok(())
    }
}

/// Create shared state
#[derive(Clone)]
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

/// Extract inner value from AppState
pub async fn with_state<T, F, R>(state: State<AppState<T>>, f: F) -> Result<R, PluginError>
where
    F: FnOnce(&mut T) -> Result<R, PluginError>,
{
    let mut guard = state.inner.lock().await;
    f(&mut *guard)
}
