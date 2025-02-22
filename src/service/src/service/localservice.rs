use log::debug;

use crate::config::LocalModelConfig;
use crate::local::manager::ModelManager;
use std::sync::Arc;

pub struct LocalService {
    model_manager: Arc<ModelManager>,
}

impl LocalService {
    pub async fn new(config: Vec<LocalModelConfig>) -> Self {
        let mut manager = ModelManager::new(config.clone());
        manager.init().await.unwrap();
        Self {
            model_manager: Arc::new(manager),
        }
    }
    pub async fn chat(&self, message: String) -> anyhow::Result<String> {
        let r = self.model_manager.submit_request(message).await;
        Ok(r.unwrap().text)
    }
    // get available runners
    pub async fn available_runners(&self) -> usize {
        let r = self.model_manager.available_runners().await;
        debug!("Available runners: {}", r);
        r
    }
}
