use async_trait::async_trait;
use protos::{model, model_ttrpc};
use std::error::Error;
use std::sync::Arc;
use ttrpc::r#async::{Server, TtrpcContext};

use crate::config::Config;

struct ModelService {}

#[async_trait]
impl model_ttrpc::ModelService for ModelService {
    async fn text_chat(
        &self,
        _ctx: &TtrpcContext,
        req: model::TextRequest,
    ) -> ttrpc::Result<model::TextResponse> {
        let mut res = model::TextResponse::default();
        res.text = req.text;
        println!("input: {}", res.text);
        Ok(res)
    }

    async fn speech_to_text(
        &self,
        _ctx: &TtrpcContext,
        req: model::SpeechRequest,
    ) -> ttrpc::Result<model::TextResponse> {
        let mut res = model::TextResponse::default();
        Ok(res)
    }
    async fn text_to_speech(
        &self,
        _ctx: &TtrpcContext,
        req: model::TextRequest,
    ) -> ttrpc::Result<model::SpeechResponse> {
        let mut res = model::SpeechResponse::default();
        Ok(res)
    }
}

pub fn remove_if_sock_exist(sock_addr: &str) -> Result<(), Box<dyn Error>> {
    let path = sock_addr
        .strip_prefix("unix://")
        .expect("socket address is not expected");

    if std::path::Path::new(path).exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub async fn start_server() -> Result<(), Box<dyn Error>> {
    let sconfig = Config::new();
    let addr = sconfig.server.addr.unwrap();
    remove_if_sock_exist(addr.as_str())?;
    let model_service = model_ttrpc::create_model_service(Arc::new(ModelService {}));
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
