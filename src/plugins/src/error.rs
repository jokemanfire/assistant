use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("插件初始化失败: {0}")]
    InitError(String),
    
    #[error("插件启动失败: {0}")]
    StartError(String),
    
    #[error("插件停止失败: {0}")]
    StopError(String),
    
    #[error("HTTP服务错误: {0}")]
    HttpError(String),
    
    #[error("请求解析错误: {0}")]
    RequestParseError(String),
    
    #[error("响应生成错误: {0}")]
    ResponseGenerationError(String),
    
    #[error("模型服务错误: {0}")]
    ModelServiceError(String),
    
    #[error("未知错误: {0}")]
    Unknown(String),
}

impl From<anyhow::Error> for PluginError {
    fn from(err: anyhow::Error) -> Self {
        PluginError::Unknown(err.to_string())
    }
}

impl From<reqwest::Error> for PluginError {
    fn from(err: reqwest::Error) -> Self {
        PluginError::HttpError(err.to_string())
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(err: serde_json::Error) -> Self {
        PluginError::RequestParseError(err.to_string())
    }
}

#[cfg(feature = "http_api")]
impl From<tonic::Status> for PluginError {
    fn from(status: tonic::Status) -> Self {
        PluginError::ModelServiceError(status.to_string())
    }
}

// 实现axum的响应转换
impl axum::response::IntoResponse for PluginError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            PluginError::RequestParseError(_) => http::StatusCode::BAD_REQUEST,
            PluginError::ModelServiceError(_) => http::StatusCode::SERVICE_UNAVAILABLE,
            _ => http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        
        let body = serde_json::json!({
            "error": {
                "message": self.to_string(),
                "type": format!("{:?}", self).split('(').next().unwrap_or("Unknown"),
            }
        });
        
        (status, axum::Json(body)).into_response()
    }
} 