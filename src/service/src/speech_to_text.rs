use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::{config::SpeechToTextConfig, server::ModelDeal};

pub struct SpeechModel{
    config: SpeechToTextConfig,
}

#[async_trait]
impl ModelDeal<Vec<u8>, String> for SpeechModel {
    async fn get_response_online(&self, data: Vec<u8>) -> Result<String, Box<dyn std::error::Error>> {
        let response = Client::new()
            .post(self.config.model_path.clone().unwrap())
            .header("Content-Type", "multipart/form-data")
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.clone().unwrap()),
            )
            .body(data)
            .send()
            .await?;
        
        let body = response.text().await?;
        println!("{}",body);
        let json_data: Value = serde_json::from_str(&body)?;
        Ok(json_data["text"].as_str().unwrap_or_default().to_string())
    }

    async fn get_response_offline(&self, _data: Vec<u8>) -> Result<String, Box<dyn std::error::Error>> {
        Ok("Offline response not implemented".to_string())
    }
}


mod tests {
    use crate::config::Config;
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_generate_response() {
        let config = Config::new();
        let dmod = SpeechModel{
            config:config.speech_to_text,
        };
        let payload = "-----011000010111000001101001\r\nContent-Disposition: form-data; name=\"file\"\r\n\r\n{}\r\n-----011000010111000001101001\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nFunAudioLLM/SenseVoiceSmall\r\n-----011000010111000001101001--\r\n\r\n";
        let r = dmod.get_response_online(payload.as_bytes().to_vec()).await;
        println!("{:?}",r);
        assert!(r.is_ok());
    }
}
