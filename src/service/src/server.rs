use prost::Message;
use ttrpc::context::Context;
use ttrpc::r#async::Server;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use protos::model_ttrpc;
use async_trait::async_trait;
use crate::config::Config;

struct ModelService;

#[async_trait]
impl model_ttrpc::ModelService for ModelService {
    // Implement the required methods here
}


async fn start_server() -> Result<(), Box<dyn Error>> {
    let sconfig = Config::new();
    let addr = sconfig.server.addr.unwrap();
    let model_service = model_ttrpc::create_model_service(Arc::new(ModelService{}));
    println!("Starting ttrpc server on {}", addr);
    let mut server = Server::new()
        .register_service(model_service)
        .bind(addr.as_str())?;
    server.start().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;


    #[tokio::test]
    async fn test_ttrpc_server() {
        let result = start_server().await;
        // println!("result: {:?}", result);
        assert!(result.is_ok());
    }
    
}