use reqwest::Client;
use serde_json::json;
use std::error::Error;

const SPEECH_TO_TEXT_API_URL: &str = "https://api.example.com/speech-to-text"; // Replace with actual URL
const API_KEY: &str = "your_api_key_here"; // Replace with your actual API key

pub async fn speech_to_text(audio_data: Vec<u8>) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let res = client.post(SPEECH_TO_TEXT_API_URL)
       .header("Authorization", format!("Bearer {}", API_KEY))
       .body(audio_data)
       .send()
       .await?;
    let text = res.text().await?;
    Ok(text)
}