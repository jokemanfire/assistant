use crate::error::PluginError;
use crate::http::AppState;
use crate::openai::OpenAIState;
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

/// 验证API密钥中间件
#[cfg(feature = "http_api")]
pub async fn verify_api_key<B>(
    State(state): State<AppState<OpenAIState>>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, PluginError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok());

    if let Some(auth) = auth_header {
        if auth.starts_with("Bearer ") {
            let api_key = auth.trim_start_matches("Bearer ").trim();
            
            // 获取配置中的API密钥
            let config = {
                let state_guard = state.inner.lock().await;
                state_guard.config.clone()
            };
            
            // 验证API密钥
            if config.api_keys.contains(&api_key.to_string()) || config.api_keys.contains(&"*".to_string()) {
                return Ok(next.run(req).await);
            }
        }
    }
    
    // 返回未授权错误
    Err(PluginError::HttpError("无效的API密钥".to_string()))
}

#[cfg(not(feature = "http_api"))]
pub async fn verify_api_key<B, S>(
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, PluginError> {
    // 如果未启用http_api特性，直接通过
    Ok(next.run(req).await)
} 