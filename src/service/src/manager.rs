use crate::runmodel::IoOption;
use crate::ModelRunner;
use std::io::Error;
use std::sync::Arc;
use std::{collections::VecDeque, f32::consts::E};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};

pub struct ModelManager {
    model_runners: Vec<(ModelRunner, Sender<String>, Receiver<String>)>,
    queue_in: mpsc::Sender<String>,
    queue_out: mpsc::Receiver<String>,
}

impl ModelManager {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(32);
        ModelManager {
            model_runners: vec![],
            queue_in: sender,
            queue_out: receiver,
        }
    }
    pub async fn create_model(&mut self) -> Result<(), Error> {
        // requeset channel
        let (request_s, request_r) = tokio::sync::mpsc::channel::<String>(256);
        // result channel
        let (result_s, result_r) = tokio::sync::mpsc::channel::<String>(256);
        let model_runner = ModelRunner {
            ioopt: IoOption {
                stdout: Some("stdout.fifo".to_string()),
                stderr: Some("stderr.fifo".to_string()),
                stdin: Some("stdin.fifo".to_string()),
            },
            model_path: "/home/hu/code/assistant/models/deepseek-ai.DeepSeek-R1-Distill-Qwen-1.5B.Q4_K_M.gguf".to_string(),
            cli_path: "llama-cli".to_string(),    
            request_receiver: Arc::new(Mutex::new(request_r)),
            result_sender: Arc::new(result_s),
            child: None,
        };
        self.model_runners.push((model_runner, request_s, result_r));
        Ok(())
    }
    pub async fn start(&mut self) -> Result<(), Error> {
        // choose a model runner from model_runners
        if self.model_runners.is_empty() {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "No model runners available",
            ));
        }
        let mut model_pack = self.model_runners.remove(0);
        model_pack.0.run_model_with_fifo().await?;
        self.model_runners.push(model_pack);
        // self.process_requests().await;
        Ok(())
    }

    pub async fn enqueue_request(&self, request: String) {
        self.queue_in
            .send(request)
            .await
            .expect("Failed to send request");
    }

    async fn process_requests(&mut self) {
        while let Some(request) = self.queue_out.recv().await {
            let _ = self.handle_request(request).await;
        }
    }

    async fn handle_request(&mut self, request: String) -> Result<String, Error> {
        // choose a model runner from model_runners
        if self.model_runners.is_empty() {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "No model runners available",
            ));
        }
        // clear the model_runner.2 inner data
        for (_, _, r) in self.model_runners.iter_mut() {
            while let Ok(_) = r.try_recv() {}
        }

        let mut model_runner = self.model_runners.remove(0);
        model_runner.1.send(request).await.unwrap();

        if let Some(r) = model_runner.2.recv().await {
            self.model_runners.push(model_runner);
            return Ok(r);
        }
        self.model_runners.push(model_runner);
        return Err(Error::new(
            std::io::ErrorKind::Other,
            "No model runners available",
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_model_manager() {
        let mut manager = ModelManager::new();
        manager.create_model().await.unwrap();
        println!("Model manager created.");
        manager.start().await.unwrap();
        println!("Model manager started.");
        tokio::time::sleep(Duration::from_secs(10)).await;
        println!("Start request--------------");
        let r = manager
            .handle_request("Hello! who are you".to_string())
            .await;
        if let Ok(r) = r {
            println!("Model manager handled request: {}", r);
        }
        tokio::time::sleep(Duration::from_secs(20)).await;
    }
}
