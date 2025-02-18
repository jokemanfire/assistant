use super::wasm_runner::WasmModelRunner;
use config::Config;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration};
use std::collections::{HashMap, VecDeque};

// 请求结构体
#[derive(Clone,Debug, Serialize, Deserialize)]
pub struct ModelRequest {
    pub prompt: String,
    pub parameters: Option<ModelParameters>,
    pub request_id: String,
}

// 模型参数
#[derive(Clone,Debug, Serialize, Deserialize)]
pub struct ModelParameters {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}

// 响应结构体
#[derive(Clone,Debug, Serialize, Deserialize)]
pub struct ModelResponse {
    pub text: String,
    pub request_id: String,
    pub status: ResponseStatus,
    pub error: Option<String>,
}

#[derive(Clone,Debug, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Error,
    Timeout,
}

#[derive(Clone)]
pub struct ModelRunner {
    wasm_runner: Arc<Mutex<WasmModelRunner>>,
    request_sender: Sender<ModelRequest>,
    result_receiver: Arc<Mutex<Receiver<ModelResponse>>>,
}

pub struct ModelManager {
    model_channel: (Arc<mpsc::Sender<ModelRunner>>, Arc<Mutex<mpsc::Receiver<ModelRunner>>>),
    config: Config,
    request_queue:(Arc<mpsc::Sender<ModelRequest>>, Arc<Mutex<mpsc::Receiver<ModelRequest>>>),
    response_senders: Arc<Mutex<Vec<(String, Sender<ModelResponse>)>>>,
}

impl ModelRunner {
    // Send request to the runner
    pub async fn get_result(&self, request: ModelRequest) -> anyhow::Result<()> {
        let response = self.result_receiver.lock().await.recv().await.unwrap();
        let runner = self.wasm_runner.lock().await.deal_request(request).await.unwrap();
        // self.result_receiver.lock().await.send(response).await.unwrap();
        Ok(())
    }
 
}
impl ModelManager {
    pub fn new(config: Config) -> Self {
        let (model_tx, model_rx) = mpsc::channel(32);
        let (request_tx, request_rx) = mpsc::channel(32);
        
        Self {
            model_channel: (Arc::new(model_tx), Arc::new(Mutex::new(model_rx))),
            config,
            request_queue: (Arc::new(request_tx), Arc::new(Mutex::new(request_rx))),
            response_senders: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        if self.config.get_bool("local_model.enabled")? {
            let model_count = self.config.get_int("local_model.instance_count").unwrap_or(1) as usize;
            let wasm_path = self.config.get_string("local_model.wasm_path")?;

            for _ in 0..model_count {
                self.create_model(&wasm_path).await?;
            }
            // 启动请求处理循环
            self.start_request_processor();
        }
        Ok(())
    }

    async fn create_model(&self, wasm_path: &str) -> anyhow::Result<()> {
        let runner = WasmModelRunner::new(HashMap::new(), wasm_path.to_string(), "llama-2-7b-chat.Q5_K_M.gguf".to_string()).unwrap();
        let (request_tx, request_rx) = mpsc::channel(32);
        let (result_tx, result_rx) = mpsc::channel(32);

        let runner = Arc::new(Mutex::new(runner));
        let runner_clone = runner.clone();
        let response_senders = self.response_senders.clone();

        tokio::spawn(async move {
            let mut rx: Receiver<ModelRequest> = request_rx;
            while let Some(request) = rx.recv().await {
                let mut runner = runner_clone.lock().await;
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

                if let Some(sender) = response_senders.lock().await.iter()
                    .find(|(id, _)| id == &request.request_id)
                    .map(|(_, sender)| sender.clone()) 
                {
                    let _ = sender.send(response).await;
                }
            }
        });

        self.model_channel.0.send(ModelRunner {
            wasm_runner: runner,
            request_sender: request_tx,
            result_receiver: Arc::new(Mutex::new(result_rx)),
        }).await?;

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

    // 外部接口: 提交请求并等待响应
    pub async fn submit_request(&self, prompt: String) -> anyhow::Result<ModelResponse> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (response_tx, mut response_rx) = mpsc::channel(1);

        // 注册响应接收器
        self.response_senders.lock().await.push((request_id.clone(), response_tx));

        // 创建请求
        let request = ModelRequest {
            prompt,
            parameters: None,
            request_id: request_id.clone(),
        };

        // 加入请求队列
        self.request_queue.0.send(request).await?;

        // 等待响应,设置超时
        match timeout(Duration::from_secs(30), response_rx.recv()).await {
            Ok(Some(response)) => {
                // 清理响应接收器
                self.response_senders.lock().await.retain(|(id, _)| id != &request_id);
                Ok(response)
            }
            _ => Ok(ModelResponse {
                text: String::new(),
                request_id,
                status: ResponseStatus::Timeout,
                error: Some("Request timeout".to_string()),
            })
        }
    }

    // 获取队列中的请求数量
    pub async fn queue_size(&self) -> usize {
        self.request_queue.1.lock().await.len()  
    }

    // 获取可用的模型运行器数量
    pub async fn available_runners(&self) -> usize {
        // 通过 try_recv 统计可用运行器数量
        let mut count = 0;
        while self.model_channel.1.lock().await.try_recv().is_ok() {
            count += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_model_manager() {
        let config = Config::builder()
            .set_default("local_model.enabled", true).unwrap()
            .set_default("local_model.instance_count", 2).unwrap()
            .set_default("local_model.wasm_path", "/home/10346053@zte.intra/hdy/wasm/wasmedge-ggml-llama.wasm").unwrap()
            .build().unwrap();

        let mut manager = ModelManager::new(config);
        manager.init().await.unwrap();
        let manager = Arc::new(Mutex::new(manager));
        // 测试并发请求
        let mut handles = vec![];
        for i in 0..3 {
            let manager = manager.clone();
            let handle = tokio::spawn(async move {
                let response = manager.lock().await.submit_request(format!("Test prompt {}", i)).await;
                println!("Response {}: {:?}", i, response);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // 检查队列状态
        println!("Queue size: {}", manager.lock().await.queue_size().await);
        println!("Available runners: {}", manager.lock().await.available_runners().await);
    }
}
