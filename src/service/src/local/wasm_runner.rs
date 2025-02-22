use crate::local::manager::ModelRequest;
use anyhow::Result;
use log::debug;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config::LocalModelConfig;

pub struct WasmModelRunner {
    config: LocalModelConfig,
    process: Option<Arc<Mutex<Child>>>,
}
/// todo use sdk
impl WasmModelRunner {
    pub fn new(
        config: LocalModelConfig,
    ) -> Result<Self> {
        Ok(Self {
            config,
            process: None,
        })
    }

    pub async fn deal_request(
        &self,
        request: ModelRequest,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(process) = &self.process {
            let mut process = process.lock().await;
            // get stdin and write request
            let stdin = process.stdin.as_mut().ok_or("Failed to get stdin")?;
            writeln!(stdin, "{}\n", serde_json::to_string(&request)?)?;
            stdin.flush()?;

            // read response from stdout
            let stdout = process.stdout.as_mut().ok_or("Failed to get stdout")?;
            let mut reader = BufReader::new(stdout);
            let mut response = Vec::new();
            reader.read_until(b'\0', &mut response)?;
            debug!("Response: {}", String::from_utf8(response.clone()).unwrap());
            Ok(String::from_utf8(response)?)
        } else {
            Err("WASM process not started".into())
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // start wasmedge process
        let process = Command::new("wasmedge")
            .arg("--dir")
            .arg(".") 
            .arg("--env")
            .arg("llama3=true")
            .arg("--env")
            .arg(format!("n_gpu_layers={}", self.config.n_gpu_layers))
            .arg("--nn-preload")
            .arg(format!("default:GGML:AUTO:{}", self.config.model_path))
            .arg(&self.config.wasm_path)
            .arg("default") // pass default as argument
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        self.process = Some(Arc::new(Mutex::new(process)));
        Ok(())
    }
}

impl Drop for WasmModelRunner {
    fn drop(&mut self) {
        if let Some(process) = &self.process {
            // try lock and kill
            if let Ok(mut process) = process.try_lock() {
                let _ = process.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run() {
        let mut runner = WasmModelRunner::new(LocalModelConfig {
            enabled: true,
            priority: 1,
            wasm_path: "/home/hu/code/assistant/target/wasm32-wasi/release/wasme-ggml.wasm".to_string(),
            model_path: "/home/hu/code/assistant/models/qwen1_5-0_5b-chat-q2_k.gguf".to_string(),
            n_gpu_layers: 0,
            ctx_size: 0,
            instance_count: 0,
        }).unwrap();
        runner.run().await.unwrap();
        let response = runner.deal_request(ModelRequest {
            prompt: "Hello, how are you?".to_string(),
            request_id: "1".to_string(),
            parameters: None,
        }).await.unwrap();
        println!("response: {}", response);
    }
}
