use crate::error::PluginError;
use crate::http::AppState;
use axum::{
    body::Body as AxumBody,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

#[cfg(feature = "http_api")]
use crate::openai::OpenAIState;

/// Verify api key middleware
#[cfg(feature = "http_api")]
pub async fn verify_api_key(
    State(state): State<AppState<OpenAIState>>,
    req: Request<AxumBody>,
    next: Next,
) -> Result<Response, PluginError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok());

    if let Some(auth) = auth_header {
        if auth.starts_with("Bearer ") {
            let api_key = auth.trim_start_matches("Bearer ").trim();

            // Get api keys from config
            let config = {
                let state_guard = state.inner.lock().await;
                state_guard.config.clone()
            };

            // Verify api key
            if config.api_keys.contains(&api_key.to_string())
                || config.api_keys.contains(&"*".to_string())
            {
                return Ok(next.run(req).await);
            }
        }
    }

    // Return unauthorized error
    Err(PluginError::HttpError("Invalid API key".to_string()))
}

#[cfg(not(feature = "http_api"))]
pub async fn verify_api_key(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, PluginError> {
    // If http_api feature is not enabled, pass directly
    Ok(next.run(req).await)
}
