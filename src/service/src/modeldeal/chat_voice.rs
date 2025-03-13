use crate::config::ChatVoiceConfig;
use reqwest::Client;
use serde_json::json;

pub struct VoiceModel {
    pub config: ChatVoiceConfig,
}

impl VoiceModel {
    pub async fn play_response(text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        let response = client
            .post("https://api.text-to-speech.com/synthesize")
            .json(&json!({
                "text": text
            }))
            .send()
            .await?;

        let audio_data = response.bytes().await?;
        //todo
        Ok(())
    }
}
