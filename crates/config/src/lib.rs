use serde::{Deserialize, Serialize};
use std::path::PathBuf;



const DEFAULT_CONFIG_PATH: &str = "/etc/assistant/config.toml";
const DEFAULT_MODEL_PATH: &str = "/etc/assistant/models";
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub scheduler: SchedulerConfig,
    pub remote_servers: Vec<RemoteServerConfig>,
    pub llama_servers: Vec<LlamaServerConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub grpc_addr: String,
    pub http_addr: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchedulerConfig {
    pub config_dir: PathBuf,
    pub max_instances: usize,
    pub max_load: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlamaServerConfig {
    pub name: String,
    pub chat_model_path: Option<String>,
    pub embedding_model_path: Option<String>,
    pub tts_model_path: Option<String>,
    pub config_path: Option<String>,
}



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteServerConfig {
    pub name: String,
    pub grpc_addr: String,
    pub weight: u32,
    pub enabled: bool,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = PathBuf::from(DEFAULT_CONFIG_PATH);
        let config_str = std::fs::read_to_string(config_path)?;
        Ok(toml::from_str(&config_str)?)
    }

    pub fn default() -> Self {
        Self {
            server: ServerConfig {
                grpc_addr: "0.0.0.0:50051".to_string(),
                http_addr: Some("0.0.0.0:8000".to_string()),
            },
            scheduler: SchedulerConfig {
                config_dir: PathBuf::from(DEFAULT_MODEL_PATH),
                max_instances: 10,
                max_load: 0.8,
            },
            remote_servers: vec![],
            llama_servers: vec![
                LlamaServerConfig {
                    name: "default".to_string(),
                    chat_model_path: Some("".to_string()),
                    embedding_model_path: Some("".to_string()),
                    tts_model_path: Some("".to_string()),
                    config_path: Some("".to_string()),
                }
            ],
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = PathBuf::from(DEFAULT_CONFIG_PATH);
        let config_str = toml::to_string_pretty(self)?;
        std::fs::write(config_path, config_str)?;
        Ok(())
    }

}

pub fn generate_example_config() -> anyhow::Result<()> {
    let config = Config::default();
    config.save()?;
    Ok(())
}

