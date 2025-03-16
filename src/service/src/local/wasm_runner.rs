use crate::config::LocalModelConfig;
use crate::local::manager::ModelRequest;
use anyhow::{anyhow, Result};
use log::debug;
use log::{error, info, warn};
use protos::grpc::model::{ChatMessage, Role};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

use super::LocalRunner;

pub struct WasmModelRunner {
    config: LocalModelConfig,
    process: Option<Arc<Mutex<Child>>>,
}

impl WasmModelRunner {
    pub fn new(config: LocalModelConfig) -> Result<Self> {
        Ok(Self {
            config,
            process: None,
        })
    }

    // Format the prompt based on the model type and messages
    fn format_prompt(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        let system_prompt = messages.iter().find(|m| m.role == Role::System as i32).map(|m| m.content.clone());

        // Format based on model type
        match self.config.model_type.as_str() {
            "deepseek-ai" | "deepseek" => {
                for msg in messages {
                    let role = match msg.role() {
                        Role::User => "User",
                        Role::Assistant => "Assistant",
                        Role::System => "System",
                        _ => "User",
                    };

                    if role == "System" {
                        prompt.push_str(&format!("[INST]{}\n[/INST]", msg.content));
                    } else {
                        prompt.push_str(&format!("[INST]{}[/INST]\n", msg.content));
                    }
                }

                // Add the assistant prefix for the response
                prompt.push_str("\n[INST]");
            }
            _ => {
                // Qwen format: <|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n{assistant}<|im_end|>
                // Add system message if available
                if let Some(system_prompt) = system_prompt {
                    prompt.push_str(&format!(
                        "<|im_start|>system\n{}\n<|im_end|>\n",
                        system_prompt
                    ));
                }else{
                    prompt.push_str(&format!(
                        "<|im_start|>system\n{}\n<|im_end|>\n",
                        "you are a helpful assistant"
                    ));
                }
                // Add conversation history
                for msg in messages
                    .iter()
                    .filter(|m| m.role != Role::System as i32)
                {
                    if msg.role == Role::User as i32 {
                        prompt.push_str(&format!(
                            "<|im_start|>user\n{}\n<|im_end|>\n",
                            msg.content
                        ));
                    } else if msg.role == Role::Assistant as i32 {
                        prompt.push_str(&format!(
                            "<|im_start|>assistant\n{}\n<|im_end|>\n",
                            msg.content
                        ));
                    }
                }

                // Add assistant prompt
                prompt.push_str("<|im_start|>assistant\n");
            }
        }

        prompt
    }

    // Process the model's response
    fn process_response(&self, response_str: &str) -> String {
        // Check if response contains error information
        if response_str.contains("bot:ERROR:") {
            let error_msg = response_str.replace("bot:ERROR:", "").trim().to_string();
            error!("WASM error: {}", error_msg);
        }

        // parse response, remove prefix
        let mut parsed_response = if response_str.starts_with("bot:") 
        {
            response_str[4..].to_string()
        } else {
            response_str.to_string()
        };

        // Check if response is empty
        if parsed_response.trim().is_empty() {
            warn!("Empty response from WASM model");
        }
        // Remove any special tokens or formatting from the response
        match self.config.model_type.to_lowercase().as_str() {
            "qwen" => {
                // Remove <|im_end|> if present
                if let Some(end_pos) = parsed_response.find("<|im_end|>") {
                    parsed_response = parsed_response[..end_pos].to_string();
                }
            }
            "deepseek" | "deepseek-ai" => {
                // Remove end markers if present
                if let Some(end_pos) = parsed_response.find("[/INST]") {
                    parsed_response = parsed_response[..end_pos].to_string();
                }
            }
            _ => {}
        }
        parsed_response
    }
}

#[async_trait::async_trait]
impl LocalRunner for WasmModelRunner {
    async fn deal_request(&self, request: ModelRequest) -> Result<String> {
        if self.process.is_none() {
            return Err(anyhow!("Process not started"));
        }
        // format prompt
        let prompt = self.format_prompt(&request.messages);
        // send prompt
        let mut process = self.process.as_ref().unwrap().lock().await;
        let stdin = process.stdin.as_mut().ok_or(anyhow!("Failed to get stdin"))?;
        debug!("formatted_prompt:\n{}", prompt);
        write!(stdin, "{}", prompt)?;
        stdin.write_all(&[0])?;
        stdin.flush()?;

        // read response
        let stdout = process.stdout.as_mut().ok_or(anyhow!("Failed to get stdout"))?;
        let mut reader = BufReader::new(stdout);
        let mut response = Vec::new();
        reader.read_until(b'\0', &mut response)?;

        // handle response
        let response_str = String::from_utf8(response)?;

        // Post-process response based on model type
        let parsed_response = self.process_response(&response_str);
        debug!("Response: {}", &parsed_response);
        Ok(parsed_response)
    }

    async fn deal_stream_request(
        &self,
        request: ModelRequest,
        sender: &Sender<String>,
    ) -> Result<()> {
        if self.process.is_none() {
            return Err(anyhow!("Process not started"));
        }

        // format prompt
        let prompt = self.format_prompt(&request.messages);
        
        // send prompt
        let mut process = self.process.as_ref().unwrap().lock().await;
        let stdin = process.stdin.as_mut().ok_or(anyhow!("Failed to get stdin"))?;
        debug!("stream formatted_prompt:\n{}", prompt);
        write!(stdin, "{}", prompt)?;
        stdin.write_all(&[0])?;
        stdin.flush()?;

        // read response in chunks
        let stdout = process.stdout.as_mut().ok_or(anyhow!("Failed to get stdout"))?;
        let mut reader = BufReader::new(stdout);
        let mut buffer = [0u8; 1]; // read one byte at a time
        let mut accumulated = Vec::new();
        
        loop {
            // read one byte
            match reader.read(&mut buffer) {
                Ok(0) => {
                    // EOF reached
                    debug!("EOF reached");
                    break;
                },
                Ok(_) => {
                    // check if null terminator is reached
                    if buffer[0] == 0 {
                        debug!("Null terminator reached");
                        break;
                    }
                    
                    // accumulate data
                    accumulated.push(buffer[0]);
                    
                    // process and send data immediately
                    if let Ok(chunk_str) = String::from_utf8(accumulated.clone()) {
                        let processed_chunk = self.process_response(&chunk_str);
                        if !processed_chunk.is_empty() {
                            if let Err(e) = sender.send(processed_chunk).await {
                                error!("Failed to send chunk: {}", e);
                                break;
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("Error reading from stdout: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        info!("Starting WASM runner with config: {:?}", self.config);

        // start wasmedge process
        let process = Command::new("wasmedge")
            .arg("--dir")
            .arg(".")
            .arg("--env")
            .arg(format!("n_gpu_layers={}", self.config.n_gpu_layers))
            .arg("--env")
            .arg(format!("model_type={}", self.config.model_type))
            .arg("--env")
            .arg(format!("ctx-size={}", self.config.ctx_size))
            .arg("--env")
            .arg(format!("stream={}", self.config.stream))
            .arg("--nn-preload")
            .arg(format!("default:GGML:AUTO:{}", self.config.model_path))
            .arg(&self.config.wasm_path)
            .arg("default") // pass default as argument
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // start a thread to listen to stderr
        let mut process_with_stderr = process;
        if let Some(stderr) = process_with_stderr.stderr.take() {
            tokio::spawn(async move {
                let stderr_reader = BufReader::new(stderr);
                for line in stderr_reader.lines() {
                    if let Ok(line) = line {
                        debug!("WASM stderr: {}", line);
                    }
                }
            });
        }

        self.process = Some(Arc::new(Mutex::new(process_with_stderr)));
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.process.is_some()
    }
}
