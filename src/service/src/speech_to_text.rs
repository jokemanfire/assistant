use reqwest::Client;
use serde_json::json;

pub async fn capture_and_convert(_stream: Vec<u8>) -> Result<String, Box<dyn std::error::Error>> {
    let audio_data = "audio_data";
    let response = Client::new()
        .post("https://api.speech-to-text.com/convert")
        .json(&json!({
            "audio": audio_data,
            "format": "wav"
        }))
        .send()
        .await?;

    let text = response.json::<serde_json::Value>().await?["text"]
        .as_str()
        .unwrap_or("No text recognized")
        .to_string();
    Ok(text)
}
