use reqwest::Client;
use serde_json::json;
use std::error::Error;

const TEXT_TO_SPEECH_API_URL: &str = "https://api.example.com/text-to-speech"; // Replace with actual API URL
const API_KEY: &str = "your_api_key"; // Replace with your actual API key

pub async fn text_to_speech(text: String) -> Result<Vec<u8>, Box<dyn Error>> {
    let client = Client::new();
    let res = client.post(TEXT_TO_SPEECH_API_URL)
        .header("Authorization", format!("Bearer {}", API_KEY))
        .json(&json!({ "text": text }))
        .send()
        .await?;
    let audio_data = res.bytes().await?.to_vec();
    Ok(audio_data)
}