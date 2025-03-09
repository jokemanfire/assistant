use crate::local::manager::ModelRequest;
use anyhow::Result;
use log::debug;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config::LocalModelConfig;
use crate::config::ChatTemplateConfig;
use protos::ttrpc::model::{ChatMessage, Role};

pub struct WasmModelRunner {
    config: LocalModelConfig,
    process: Option<Arc<Mutex<Child>>>,
    chat_templates: ChatTemplateConfig,
}
/// todo change touse sdk
impl WasmModelRunner {
    pub fn new(
        config: LocalModelConfig,
    ) -> Result<Self> {
        let chat_templates = ChatTemplateConfig::new();
        Ok(Self {
            config,
            process: None,
            chat_templates,
        })
    }

    pub async fn deal_request(
        &self,
        request: ModelRequest,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(process) = &self.process {
            let mut process = process.lock().await;
            debug!("model_type: {}", self.config.model_type);
            // get template
            let template = self.chat_templates.get_template(&self.config.model_type.to_lowercase());
            
            // Create conversation content
            let mut conversation_content = String::new();
            
            // Get system prompt (if any)
            let system_prompt = request.system_prompt.clone().unwrap_or_else(|| {
                // Find system message in messages
                request.messages.iter()
                    .find(|msg| msg.role == Role::ROLE_SYSTEM.into())
                    .map(|msg| msg.content.clone())
                    .unwrap_or_default()
            });
            
            // Add system message
            if !system_prompt.is_empty() {
                let system_msg = template.system.replace("{system_prompt}", &system_prompt);
                conversation_content.push_str(&system_msg);
                conversation_content.push_str("\n");
            }
            
            // Add user and assistant messages in order (ignore system message, as it was handled separately)
            for msg in request.messages.iter().filter(|m| m.role != Role::ROLE_SYSTEM.into()) {
                if msg.role == Role::ROLE_USER.into() {
                    let user_msg = template.user.replace("{prompt}", &msg.content);
                    conversation_content.push_str(&user_msg);
                    conversation_content.push_str("\n");
                } else if msg.role == Role::ROLE_ASSISTANT.into() {
                    let assistant_msg = template.assistant.replace("{content}", &msg.content);
                    conversation_content.push_str(&assistant_msg);
                    conversation_content.push_str("\n");
                }
            }
            
            // Add final assistant prompt to indicate that the model should respond
            conversation_content.push_str(&template.assistant);
            
            // Format final prompt based on model type
            let formatted_prompt = match self.config.model_type.to_lowercase().as_str() {
                "qwen" => {
                    // Qwen format: <|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n{assistant}<|im_end|>
                    let mut prompt = String::new();
                    
                    // Add system message if available
                    if !system_prompt.is_empty() {
                        prompt.push_str(&format!("<|im_start|>system\n{}\n<|im_end|>\n", system_prompt));
                    }
                    
                    // Add conversation history
                    for msg in request.messages.iter().filter(|m| m.role != Role::ROLE_SYSTEM.into()) {
                        if msg.role == Role::ROLE_USER.into() {
                            prompt.push_str(&format!("<|im_start|>user\n{}\n<|im_end|>\n", msg.content));
                        } else if msg.role == Role::ROLE_ASSISTANT.into() {
                            prompt.push_str(&format!("<|im_start|>assistant\n{}\n<|im_end|>\n", msg.content));
                        }
                    }
                    
                    // Add assistant prompt
                    prompt.push_str("<|im_start|>assistant\n");
                    prompt
                },
                "deepseek" | "deepseek-ai" => {
                    // DeepSeek format: <|system|>\n{system}\n<|user|>\n{user}\n<|assistant|>\n{assistant}
                    let mut prompt = String::new();
                    
                    // Add system message if available
                    if !system_prompt.is_empty() {
                        prompt.push_str(&format!("<|system|>\n{}\n", system_prompt));
                    }
                    
                    // Add conversation history
                    for msg in request.messages.iter().filter(|m| m.role != Role::ROLE_SYSTEM.into()) {
                        if msg.role == Role::ROLE_USER.into() {
                            prompt.push_str(&format!("<|user|>\n{}\n", msg.content));
                        } else if msg.role == Role::ROLE_ASSISTANT.into() {
                            prompt.push_str(&format!("<|assistant|>\n{}\n", msg.content));
                        }
                    }
                    
                    // Add assistant prompt
                    prompt.push_str("<|assistant|>\n");
                    prompt
                },
                _ => {
                    // Generic template fallback using conversation template
                    template.chat_template.replace("{conversation}", &conversation_content)
                }
            };
            
            // send prompt
            let stdin = process.stdin.as_mut().ok_or("Failed to get stdin")?;
            debug!("formatted_prompt: {}", formatted_prompt);
            write!(stdin, "{}", formatted_prompt)?;
            stdin.write_all(&[0])?;
            stdin.flush()?;
            
            // read response
            let stdout = process.stdout.as_mut().ok_or("Failed to get stdout")?;
            let mut reader = BufReader::new(stdout);
            let mut response = Vec::new();
            reader.read_until(b'\0', &mut response)?;
            
            // handle response
            let response_str = String::from_utf8(response)?;
            
            // parse response, remove prefix
            let mut parsed_response = if response_str.starts_with("bot:") {
                response_str[4..].to_string()
            } else {
                response_str
            };
            
            // Post-process response based on model type
            match self.config.model_type.to_lowercase().as_str() {
                "qwen" => {
                    // Remove <|im_end|> if present
                    if let Some(end_pos) = parsed_response.find("<|im_end|>") {
                        parsed_response = parsed_response[..end_pos].to_string();
                    }
                },
                "deepseek" | "deepseek-ai" => {
                    // Remove end markers if present
                    if let Some(end_pos) = parsed_response.find(" ") {
                        parsed_response = parsed_response[..end_pos].to_string();
                    }
                },
                _ => {}
            }
            
            debug!("Response: {}", &parsed_response);
            Ok(parsed_response)
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
            .arg(format!("n_gpu_layers={}", self.config.n_gpu_layers))
            .arg("--env")
            .arg(format!("model_type={}", self.config.model_type))
            .arg("--env")
            .arg(format!("ctx-size={}", self.config.ctx_size))
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
    use protos::ttrpc::model::{ChatMessage, Role};

    use super::*;

    #[tokio::test]
    async fn test_run_qwen() {
        println!("test_run");
        let mut runner = WasmModelRunner::new(LocalModelConfig {
            enabled: true,
            priority: 1,
            wasm_path: "/home/hu/code/assistant/target/wasm32-wasi/release/wasme-ggml.wasm".to_string(),
            model_path: "/home/hu/code/assistant/models/qwen1_5-0_5b-chat-q2_k.gguf".to_string(),
            n_gpu_layers: 0,
            ctx_size: 1024,
            instance_count: 0,
            model_type: "qwen".to_string(),
            stream: true,
        }).unwrap();

        runner.run().await.unwrap();
        let response = runner.deal_request(ModelRequest {
            messages: vec![ChatMessage {
                role: Role::ROLE_USER.into(),
                content: "Hello, how are you?".to_string(),
                ..Default::default()
            }],
            request_id: "1".to_string(),
            parameters: None,
            system_prompt: Some("You are a helpful assistant.".to_string()),
        }).await.unwrap();
        println!("get result from wasm: {}", response);
    }
    #[tokio::test]
    async fn test_run_deepseek() {
        println!("test_run_deepseek");
        let mut runner = WasmModelRunner::new(LocalModelConfig {
            enabled: true,
            priority: 1,
            wasm_path: "/home/hu/code/assistant/target/wasm32-wasi/release/wasme-ggml.wasm".to_string(),
            model_path: "/home/hu/code/assistant/models/deepseek-ai.DeepSeek-R1-Distill-Qwen-1.5B.Q4_K_M.gguf".to_string(),
            n_gpu_layers: 0,
            ctx_size: 2048,
            instance_count: 0,
            model_type: "deepseek-ai".to_string(),
            stream: true,
        }).unwrap();
        runner.run().await.unwrap();
        let response = runner.deal_request(ModelRequest {
            messages: vec![ChatMessage {
                role: Role::ROLE_USER.into(),
                content: "Hello, how are you?".to_string(),
                ..Default::default()
            }],
            request_id: "1".to_string(),
            parameters: None,
            system_prompt: Some("You are a helpful assistant.".to_string()),
        }).await.unwrap();
        println!("get result from wasm: {}", response);
    }
}
