use crate::config::Config;
use reqwest::Client;
use serde_json::json;
use serde_json::Value;
pub async fn generate_response(text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let cfg = Config::new();
    let response = client
        .post(cfg.dialogue_model.model_path.unwrap())
        .header("Content-Type", "application/json")
        .header(
            "Authorization",
            format!("Bearer {}", cfg.dialogue_model.api_key.unwrap()),
        )
        .json(&json!({
            "model": cfg.dialogue_model.model_name.unwrap(),
            "messages": [
                {
                    "role": "system",
                    "content": "你是一个女仆，请控制对话到10字以内"
                },
                {
                    "role": "user",
                    "content": text
                }
            ]
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Request failed with status code: {}", response.status()).into());
    }

    let body = response.text().await?;
    let json_data: Value = serde_json::from_str(&body)?;
    println!("{:?}", json_data);
    let response_text = json_data["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("Failed to extract response text")?;
    Ok(response_text.to_string())
}

#[cfg(test)]

mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_generate_response() {
        let text = "Hello, how are you?";
        let response = generate_response(text).await.unwrap();
        // assert_eq!(response, "I'm fine, thank you. How are you?");
        println!("{}", response);
    }
}
