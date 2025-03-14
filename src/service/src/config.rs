use log;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

const DEFAULT_CONFIG: &str = include_str!("default.toml");

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    pub server: ServerConfig,
    pub remote_server: RemoteServerConfig,
    pub voice_chat: VoiceChatConfig,
    pub chat_model: ChatConfig,
    pub chat_voice: ChatVoiceConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ServerConfig {
    pub grpc_addr: Option<String>,
    pub http_addr: Option<String>,
    pub try_max_time: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RemoteServerConfig {
    pub endpoints: Vec<String>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ModelConfig {
    pub enabled: bool,
    pub priority: i32, // priority, the smaller the number, the higher the priority
    pub model_path: String,
    pub model_name: String,
    pub api_key: String,
    pub stream: bool,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LocalModelConfig {
    pub enabled: bool,
    pub priority: i32,
    pub wasm_path: String,
    pub model_path: String,
    pub n_gpu_layers: i32,
    pub ctx_size: i32,
    pub instance_count: i32,
    pub model_type: String,
    pub stream: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct VoiceChatConfig {
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChatConfig {
    pub knowledge_base: Option<String>,
    pub local_models: Vec<LocalModelConfig>,
    pub remote_models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChatVoiceConfig {
    pub model_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChatTemplateConfig {
    pub templates: HashMap<String, ModelTemplate>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ModelTemplate {
    pub user: String,
    pub assistant: String,
    pub system: String,
    pub chat_template: String,
}

impl Config {
    pub fn new() -> Self {
        let config_path = "/etc/assistant/service/config.toml";

        // Get config from file
        if let Ok(config_str) = fs::read_to_string(config_path) {
            if let Ok(config) = toml::from_str(&config_str) {
                return config;
            }
        }

        // If failed to load, use default config
        let default_config = include_str!("default.toml");
        toml::from_str(default_config).unwrap_or_else(|_| {
            log::error!("Failed to parse default config");
            Self::default()
        })
    }
}

impl ChatTemplateConfig {
    pub fn new() -> Self {
        // 直接在代码中定义模板，不再从文件加载
        let mut templates = HashMap::new();

        // Qwen chat template
        templates.insert(
            "qwen".to_string(),
            ModelTemplate {
                user: "<|im_start|>user\n{prompt}\n<|im_end|>".to_string(),
                assistant: "<|im_start|>assistant\n".to_string(),
                system: "<|im_start|>system\n{system_prompt}<|im_end|>".to_string(),
                chat_template: "{system}\n{user}\n{assistant}".to_string(),
            },
        );

        // DeepSeek chat template
        templates.insert(
            "deepseek".to_string(),
            ModelTemplate {
                user: "Human: {prompt}\n".to_string(),
                assistant: "Assistant: ".to_string(),
                system: "{system_prompt}\n".to_string(),
                chat_template: "{system}{user}{assistant}".to_string(),
            },
        );

        // DeepSeek-AI chat template (same as DeepSeek)
        templates.insert(
            "deepseek-ai".to_string(),
            ModelTemplate {
                user: "Human: {prompt}\n".to_string(),
                assistant: "Assistant: ".to_string(),
                system: "{system_prompt}\n".to_string(),
                chat_template: "{system}{user}{assistant}".to_string(),
            },
        );

        // Default chat template
        templates.insert(
            "default".to_string(),
            ModelTemplate {
                user: "{prompt}".to_string(),
                assistant: "{response}".to_string(),
                system: "{system_prompt}".to_string(),
                chat_template: "{user}".to_string(),
            },
        );

        Self { templates }
    }

    // Get template for specified model
    pub fn get_template(&self, model_type: &str) -> &ModelTemplate {
        self.templates.get(model_type).unwrap_or_else(|| {
            // If no template for specified model, return default template
            self.templates.get("default").unwrap_or_else(|| {
                static DEFAULT: Lazy<ModelTemplate> = Lazy::new(|| ModelTemplate::default());
                &DEFAULT
            })
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    #[test]
    fn test_config() {
        let config = Config::new();
        println!("{:?}", config);
    }
}
