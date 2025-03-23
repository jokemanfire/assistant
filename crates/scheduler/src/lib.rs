use anyhow::Result;
use config::LlamaServerConfig;
use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};
use uuid::Uuid;
use std::sync::Arc;
use toml;
use tonic::Status;
use protos::assistant::Response;

const DEFAULT_LLAMA_WASM_PATH:&str = "/etc/assistant/bin/llama-api-server.wasm";
const DEFAULT_WASM_URL:&str = "https://github.com/LlamaEdge/LlamaEdge/releases/latest/download/llama-api-server.wasm";

#[derive(Debug, Deserialize)]
struct TomlConfig {
    server: ServerConfig,
    chat: Option<ChatConfig>,
    embedding: Option<EmbeddingConfig>,
    tts: Option<TtsConfig>,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    socket_addr: String,
}

#[derive(Debug, Deserialize)]
struct ChatConfig {
}

#[derive(Debug, Deserialize)]
struct EmbeddingConfig {
}

#[derive(Debug, Deserialize)]
struct TtsConfig {
}

// Service instance running llama-api-server
#[derive(Debug, Clone)]
pub struct ServiceInstance {
    pub id: String,
    pub config: LlamaServerConfig,
    pub server_addr: String,
    pub status: ServiceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceStatus {
    Starting,
    Running,
    Failed,
    Stopped,
}

// Scheduler manages multiple llama-api-server instances
pub struct Scheduler {
    instances: Arc<RwLock<HashMap<String, ServiceInstance>>>,
    config_dir: PathBuf,
    max_instances: usize,
}

impl Scheduler {
    pub fn new(config_dir: PathBuf, max_instances: usize) -> Self {
        Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            config_dir,
            max_instances,
        }
    }

    // Load and start all model instances from config directory
    pub async fn load_instances(&self, configs: Vec<LlamaServerConfig>) -> Result<()> {
        for config in configs {
            if let Err(e) = self.start_instance_with_config(config.clone()).await {
                warn!("Failed to start instance from {:?}: {}", config.name, e);
            }
        }
        Ok(())
    }

    // Start a new llama-api-server instance with config
    pub async fn start_instance_with_config(&self, config: LlamaServerConfig) -> Result<ServiceInstance> {
        if self.instances.read().await.len() >= self.max_instances {
            // Check max instances
            return Err(anyhow::anyhow!("Maximum number of instances reached"));
        }
        
       

        // Create instance ID
        let id = Uuid::new_v4().to_string();
        
        // Create config directory if it doesn't exist
        std::fs::create_dir_all(&self.config_dir)?;
        
        // Read server address from config file
        let config_path = config.config_path.clone().unwrap_or("".to_string());
        let server_addr = if !config_path.is_empty() {
            let config_content = std::fs::read_to_string(&config_path)?;
            let toml_config: TomlConfig = toml::from_str(&config_content)?;
            toml_config.server.socket_addr
        } else {
            panic!("Config file not found");
        };
        
        // Create instance
        let instance = ServiceInstance {
            id: id.clone(),
            config: config.clone(),
            server_addr,
            status: ServiceStatus::Starting,
        };
     
        // Start llama-api-server process
        tokio::spawn({
            let config_path = config.config_path.clone().unwrap_or("".to_string());
            let id = id.clone();
            let instances = self.instances.clone();
            async move {
                debug!("Starting llama-api-server with config: {:?}", config_path);
                // if file does not exist, download it
                // create the directory if it doesn't exist
                let dir = Path::new(DEFAULT_LLAMA_WASM_PATH).parent().unwrap();
                std::fs::create_dir_all(dir).unwrap();
                if !Path::new(DEFAULT_LLAMA_WASM_PATH).exists() {
                    let response = reqwest::get(DEFAULT_WASM_URL).await.unwrap();
                    let body = response.bytes().await.unwrap();
                    std::fs::write(DEFAULT_LLAMA_WASM_PATH, body).unwrap();
                }
                // command to run the wasm file
                let chat_model_path = config.chat_model_path.clone().unwrap_or("".to_string());
                let embedding_model_path = config.embedding_model_path.clone();
                let tts_model_path = config.tts_model_path.clone();

                let mut command = Command::new("wasmedge");
                
                // get absolute path of config file
                let config_path = if config_path.starts_with('/') {
                    PathBuf::from(config_path)
                } else {
                    std::env::current_dir().unwrap().join(config_path)
                };

                let default_dir = PathBuf::from(".");
                let work_dir = config_path.parent().unwrap_or(&default_dir);
                command.current_dir(work_dir);

                command.arg("--dir")
                    .arg(".:.")
                    .arg("--nn-preload")
                    .arg(format!("default:GGML:AUTO:{}", chat_model_path));

                let mut embedding = false;
                let mut tts = false;
                if let Some(embedding_path) = embedding_model_path {
                    if !embedding_path.is_empty() {
                        command.arg("--nn-preload")
                            .arg(format!("embedding:GGML:AUTO:{}", embedding_path));
                        embedding = true;
                    }
                }

                if let Some(tts_path) = tts_model_path {
                    if !tts_path.is_empty() {
                        command.arg("--nn-preload")
                            .arg(format!("tts:GGML:AUTO:{}", tts_path));
                        tts = true;
                    }
                }

                debug!("Config file path: {:?}", config_path);
                debug!("Working directory: {:?}", work_dir);
                // only need config file name
                let config_file_name = config_path.file_name().unwrap().to_str().unwrap();
                command.arg(DEFAULT_LLAMA_WASM_PATH)
                    .arg("config")
                    .arg("--file")
                    .arg(config_file_name)
                    .arg("--chat");

                if embedding {
                    command.arg("--embedding");
                }
                if tts {
                    command.arg("--tts");
                }
    
                // create a channel to send status to main thread
                let (tx, mut rx) = tokio::sync::mpsc::channel(1);
                let instances = instances.clone();
                let id_clone = id.clone();
                debug!("change status to running");
                if let Some(instance) = instances.write().await.get_mut(&id_clone) {
                    instance.status = ServiceStatus::Running;
                }
                // start a task to monitor the status of the instance
                let monitor_handle = tokio::spawn(async move {
                    debug!("Waiting for status from llama-api-server");
                    while let Some(_line) = rx.recv().await {
                        let mut instances = instances.write().await;
                        if let Some(instance) = instances.get_mut(&id_clone) {
                            instance.status = ServiceStatus::Stopped;
                        }
                    }
                });

                // not print command's log
                command.stdout(Stdio::null());
                debug!("Running command: {:?}", command);
                let status = command.status();
                debug!("Command status: {:?}", status);
                
                // send a signal to the monitor task
                let _ = tx.send(()).await;
                let _ = monitor_handle.await;
            }
        });

        self.instances.write().await.insert(id.clone(), instance.clone());
        
        Ok(instance)
    }


    // Stop a running instance
    pub async fn stop_instance(&self, id: &str) -> Result<()> {
        let mut instances = self.instances.write().await;
        
        if let Some(instance) = instances.get_mut(id) {
            // Send termination signal
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg(id)
                .status();
                
            instance.status = ServiceStatus::Stopped;
            instances.remove(id);
            
            // Remove config file
            let config_path = self.config_dir.join(format!("{}.toml", id));
            if config_path.exists() {
                std::fs::remove_file(config_path)?;
            }
        }
        
        Ok(())
    }

    // Get instance by ID
    pub async fn get_instance(&self, id: &str) -> Option<ServiceInstance> {
        let instances = self.instances.read().await;
        instances.get(id).cloned()
    }

    // Get all instances
    pub async fn list_instances(&self) -> Vec<ServiceInstance> {
        let instances = self.instances.read().await;
        instances.values().cloned().collect()
    }

    // check current load status
    pub async fn check_load(&self) -> f32 {
        let instances = self.instances.read().await;
        let running_instances = instances.values()
            .filter(|i| i.status == ServiceStatus::Running)
            .count();
        
        running_instances as f32 / self.max_instances as f32
    }

    // check if busy
    pub async fn is_busy(&self, max_load: f32) -> bool {
        self.check_load().await >= max_load
    }

    // Forward request to appropriate instance or return error if too busy
    pub async fn forward_request(&self,path: &str, method: &str, body: Vec<u8>, headers: HashMap<String, String>) -> Result<(u16, Vec<u8>, HashMap<String, String>)> {
        let instances = self.instances.read().await;
        // debug!("Instances: {:?}", instances);
        let instance = instances.values()
            .find(|i| i.status == ServiceStatus::Running)
            .ok_or_else(|| anyhow::anyhow!("No available running instances"))?;
        // debug!("Instance: {:?}", instance);
        // no proxy
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()?;
        
        let url = format!("http://{}{}", instance.server_addr, path);
        // debug!("Forwarding request to: {}", url);
        let mut request = client
            .request(reqwest::Method::from_bytes(method.as_bytes())?, url)
            .body(body);
            
        // Add headers
        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;

        let status = response.status().as_u16();
        let headers = response.headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = response.bytes().await?.to_vec();
        // debug!("Response body: {:?}", String::from_utf8_lossy(&body));
        Ok((status, body, headers))
    }

    // Forward request stream to appropriate instance
    pub async fn forward_request_stream(
        &self,
        path: &str,
        method: &str,
        body: Vec<u8>,
        headers: HashMap<String, String>,
        tx: mpsc::Sender<Result<Response, Status>>,
    ) -> Result<()> {
        let instances = self.instances.read().await;
        let instance = instances.values()
            .find(|i| i.status == ServiceStatus::Running)
            .ok_or_else(|| anyhow::anyhow!("No available running instances"))?;

        let client = reqwest::Client::builder()
            .no_proxy()
            .build()?;
        
        let url = format!("http://{}{}", instance.server_addr, path);
        // debug!("Forwarding stream request to: {}", url);
        
        let mut request = client
            .request(reqwest::Method::from_bytes(method.as_bytes())?, url)
            .body(body);
            
        // Add headers
        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        let status = response.status().as_u16();
        let headers = response.headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response.bytes().await?.to_vec();
        let _ = tx.send(Ok(Response {
            status: status as i32,
            body,
            headers,
        })).await;

        Ok(())
    }
} 