use super::wasm_runner::WasmModelRunner;
use super::LocalRunner;
use crate::config::LocalModelConfig;
use protos::grpc::model::ChatMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct ModelRequest {
    pub messages: Vec<ChatMessage>,
    pub request_id: String,
    pub parameters: Option<HashMap<String, String>>,
    pub system_prompt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelResponse {
    pub text: String,
    pub request_id: String,
    pub status: ResponseStatus,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Error,
    Timeout,
}

#[derive(Clone)]
pub struct ModelRunner {
    runner: Arc<Mutex<Box<dyn LocalRunner + Send>>>,
    request_sender: Sender<ModelRequest>,
    result_receiver: Arc<Mutex<Receiver<ModelResponse>>>,
}

pub struct ModelManager {
    model_channel: (
        Arc<mpsc::Sender<ModelRunner>>,
        Arc<Mutex<mpsc::Receiver<ModelRunner>>>,
    ),
    configs: Vec<LocalModelConfig>,
    request_queue: (
        Arc<mpsc::Sender<ModelRequest>>,
        Arc<Mutex<mpsc::Receiver<ModelRequest>>>,
    ),
    response_senders: Arc<Mutex<HashMap<String, Sender<ModelResponse>>>>,
    stream_response_senders: Arc<Mutex<HashMap<String, Sender<String>>>>,
}

impl ModelManager {
    pub fn new(configs: Vec<LocalModelConfig>) -> Self {
        let (model_tx, model_rx) = mpsc::channel(32);
        let (request_tx, request_rx) = mpsc::channel(32);

        Self {
            model_channel: (Arc::new(model_tx), Arc::new(Mutex::new(model_rx))),
            configs,
            request_queue: (Arc::new(request_tx), Arc::new(Mutex::new(request_rx))),
            response_senders: Arc::new(Mutex::new(HashMap::new())),
            stream_response_senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        for c in &self.configs {
            self.create_model(&c).await?;
        }
        // start request process
        self.start_request_processor();
        Ok(())
    }

    async fn create_model(&self, wasm_config: &LocalModelConfig) -> anyhow::Result<()> {
        let mut runner: Box<dyn LocalRunner + Send> =
            Box::new(WasmModelRunner::new(wasm_config.clone())?);
        // start runner
        runner.run().await.unwrap();
        let (request_tx, request_rx) = mpsc::channel(32);
        let (_result_tx, result_rx) = mpsc::channel(32);

        let runner = Arc::new(Mutex::new(runner));
        let runner_clone = runner.clone();
        let response_senders = self.response_senders.clone();
        let stream_response_senders = self.stream_response_senders.clone();

        tokio::spawn(async move {
            let mut rx: Receiver<ModelRequest> = request_rx;
            // If get the model request, deal with it
            while let Some(request) = rx.recv().await {
                let runner = runner_clone.lock().await;

                // Check if this is a streaming request by looking for a stream sender
                let is_streaming = stream_response_senders
                    .lock()
                    .await
                    .contains_key(&request.request_id);

                if is_streaming {
                    // Handle streaming request
                    if let Some(sender) = stream_response_senders
                        .lock()
                        .await
                        .get_mut(&request.request_id)
                        .map(|sender| sender.clone())
                    {
                        match runner.deal_stream_request(request.clone(), &sender).await {
                            Ok(_) => {
                                // Streaming completed successfully
                            }
                            Err(e) => {
                                // Send error message to the stream
                                let _ = sender.send(format!("Error: {}", e)).await;
                            }
                        }
                    }
                } else {
                    // Handle regular request
                    let response = match runner.deal_request(request.clone()).await {
                        Ok(text) => ModelResponse {
                            text,
                            request_id: request.request_id.clone(),
                            status: ResponseStatus::Success,
                            error: None,
                        },
                        Err(e) => ModelResponse {
                            text: String::new(),
                            request_id: request.request_id.clone(),
                            status: ResponseStatus::Error,
                            error: Some(e.to_string()),
                        },
                    };
                    // If get the response, send to the target id response sender
                    if let Some(sender) = response_senders
                        .lock()
                        .await
                        .get_mut(&request.request_id)
                        .map(|sender| sender.clone())
                    {
                        let _ = sender.send(response).await;
                    }
                }

                // Clean up the sender after processing
                stream_response_senders
                    .lock()
                    .await
                    .remove(&request.request_id);
            }
        });

        self.model_channel
            .0
            .send(ModelRunner {
                runner,
                request_sender: request_tx,
                result_receiver: Arc::new(Mutex::new(result_rx)),
            })
            .await?;

        Ok(())
    }

    fn start_request_processor(&self) {
        let (request_tx, request_rx) = self.request_queue.clone();
        let (model_tx, model_rx) = self.model_channel.clone();

        tokio::spawn(async move {
            loop {
                let mut request_rx = request_rx.lock().await;
                let mut model_rx = model_rx.lock().await;
                tokio::select! {
                    Some(request) = request_rx.recv() => {
                        if let Some(runner) = model_rx.recv().await {
                            let _ = runner.request_sender.send(request).await;
                            let _ = model_tx.send(runner).await;
                        } else {
                            let _ = request_tx.send(request).await;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(10)) => continue,
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    }

    pub async fn submit_request(
        &self,
        messages: Vec<ChatMessage>,
    ) -> anyhow::Result<ModelResponse> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (response_tx, mut response_rx) = mpsc::channel(1);

        // register response receiver
        self.response_senders
            .lock()
            .await
            .insert(request_id.clone(), response_tx);

        // create request
        let request = ModelRequest {
            messages,
            parameters: None,
            request_id: request_id.clone(),
            system_prompt: None,
        };

        // add request to queue
        self.request_queue.0.send(request).await?;

        // wait response, set timeout
        match timeout(DEFAULT_TIMEOUT, response_rx.recv()).await {
            Ok(Some(response)) => {
                self.response_senders.lock().await.remove(&request_id);
                Ok(response)
            }
            _ => {
                self.response_senders.lock().await.remove(&request_id);
                Ok(ModelResponse {
                    text: String::new(),
                    request_id,
                    status: ResponseStatus::Timeout,
                    error: Some("Request timeout".to_string()),
                })
            }
        }
    }

    pub async fn submit_stream_request(
        &self,
        messages: Vec<ChatMessage>,
    ) -> anyhow::Result<mpsc::Receiver<String>> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (stream_tx, stream_rx) = mpsc::channel(32);

        // register stream response receiver
        self.stream_response_senders
            .lock()
            .await
            .insert(request_id.clone(), stream_tx);

        // create request
        let request = ModelRequest {
            messages,
            parameters: None,
            request_id: request_id.clone(),
            system_prompt: None,
        };

        // add request to queue
        self.request_queue.0.send(request).await?;

        Ok(stream_rx)
    }

    // get available runners
    pub async fn available_runners(&self) -> usize {
        self.model_channel.1.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protos::grpc::model::Role;
    use tokio;

    #[tokio::test]
    async fn test_model_manager() {
        let config = LocalModelConfig {
            enabled: true,
            priority: 1,
            wasm_path: "/home/hu/code/assistant/target/wasm32-wasi/release/wasme-ggml.wasm".to_string(),
            model_path: "/home/hu/code/assistant/models/deepseek-ai.DeepSeek-R1-Distill-Qwen-1.5B.Q4_K_M.gguf".to_string(),
            n_gpu_layers: 0,
            ctx_size: 0,
            instance_count: 0,
            model_type: "deepseek-ai".to_string(),
            ..Default::default()
        };
        let mut manager = ModelManager::new(vec![config]);
        manager.init().await.unwrap();

        let response1 = manager
            .submit_request(vec![ChatMessage {
                role: Role::User.into(),
                content: "你好".to_string(),
                ..Default::default()
            }])
            .await
            .unwrap();
        println!("Response1: {}", response1.text);
        let response2 = manager
            .submit_request(vec![ChatMessage {
                role: Role::User.into(),
                content: "你是谁".to_string(),
                ..Default::default()
            }])
            .await
            .unwrap();
        println!("Response2: {}", response2.text);
    }
}
