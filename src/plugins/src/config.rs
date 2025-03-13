use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// HTTP服务监听地址
    pub host: String,
    /// HTTP服务监听端口
    pub port: u16,
    /// 是否启用CORS
    pub enable_cors: bool,
    /// 是否启用日志
    pub enable_logging: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8000,
            enable_cors: true,
            enable_logging: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// 是否启用OpenAI兼容API
    pub enabled: bool,
    /// API密钥验证
    pub api_keys: Vec<String>,
    /// 模型映射，将OpenAI模型名映射到本地模型
    pub model_mapping: std::collections::HashMap<String, String>,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        let mut model_mapping = std::collections::HashMap::new();
        model_mapping.insert("gpt-3.5-turbo".to_string(), "default".to_string());
        model_mapping.insert("gpt-4".to_string(), "default".to_string());
        model_mapping.insert("gpt-4o".to_string(), "default".to_string());
        
        Self {
            enabled: true,
            api_keys: vec!["YOUR_API_KEY".to_string()],
            model_mapping,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    pub http: HttpConfig,
    pub openai: OpenAIConfig,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            http: HttpConfig::default(),
            openai: OpenAIConfig::default(),
        }
    }
} 