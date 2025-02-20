use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const DEFAULT_CONFIG: &str = include_str!("default.toml");

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    pub server: ServerConfig,
    pub remote_server: RemoteServerConfig,
    pub speech_to_text: SpeechToTextConfig,
    pub dialogue_model: DialogueModelConfig,
    pub text_to_speech: TextToSpeechConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ServerConfig {
    pub ttrpc_addr: Option<String>,
    pub grpc_addr: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RemoteServerConfig {
    pub endpoints: Vec<String>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ModelConfig {
    pub enabled: bool,
    pub priority: i32, // 优先级，数字越小优先级越高
    pub model_path: String,
    pub model_name: String,
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
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SpeechToTextConfig {
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DialogueModelConfig {
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub api_key: Option<String>,
    pub stream: bool,
    pub prompt_path: Option<String>,
    pub local_models: Vec<LocalModelConfig>,
    pub remote_models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TextToSpeechConfig {
    pub model_path: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        toml::from_str(DEFAULT_CONFIG).unwrap()
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
