use reqwest::Client;
use serde_json::json;

pub async fn generate_response(text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let response = client
        .post("https://ark.cn-beijing.volces.com/api/v3/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer {token}")
        .json(&json!({
            "model": "ep-20250205152535-z4rzh",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful assistant."
                },
                {
                    "role": "user",
                    "content": text
                }
            ]
        }))
        .send()
        .await?;
    println!("{:?}",response);
    let body = response.text().await?;
    println!("{:?}",body);
    let response_text = body;
    Ok(response_text)
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
        println!("{}",response);
    }
}