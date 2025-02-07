use async_trait::async_trait;
use protos::{model, model_ttrpc};
use std::error::Error;
use std::sync::Arc;
use ttrpc::r#async::Server;

use crate::config::Config;
use crate::dialogue_model;

struct ModelS;

#[async_trait]
impl model_ttrpc::ModelService for ModelS {
    async fn text_chat(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        req: model::TextRequest,
    ) -> ::ttrpc::Result<model::TextResponse> {
        let mut res = model::TextResponse::default();
        let text_data = req.text;
        let r = dialogue_model::generate_response(&text_data).await;
        // println!("input: {}", res.text);
        if let Ok(r) = r {
            res.text = r;
        }
        Ok(res)
    }

    async fn speech_to_text(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        _req: model::SpeechRequest,
    ) -> ::ttrpc::Result<model::TextResponse> {
        let res = model::TextResponse::default();
        Ok(res)
    }
    async fn text_to_speech(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        _req: model::TextRequest,
    ) -> ::ttrpc::Result<model::SpeechResponse> {
        let res = model::SpeechResponse::default();
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

pub async fn start_server() -> Result<Server, Box<dyn Error>> {
    let sconfig = Config::new();
    let addr = sconfig.server.addr.unwrap();
    remove_if_sock_exist(addr.as_str())?;

    let model_service = model_ttrpc::create_model_service(Arc::new(ModelS {}));
    println!("Starting ttrpc server on {}", addr);
    let mut server = Server::new()
        .bind(addr.as_str())
        .unwrap()
        .register_service(model_service);

    server.start().await.unwrap();
    Ok(server)
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
