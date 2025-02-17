use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use crate::{config::SpeechToTextConfig, server::ModelDeal};

pub struct SpeechModel {
    config: SpeechToTextConfig,
}

#[async_trait]
impl ModelDeal<Vec<u8>, String> for SpeechModel {
    async fn get_response_online(
        &self,
        data: Vec<u8>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(data)
                    .file_name("test.wav")
                    .mime_str("audio/wav")
                    .unwrap(),
            )
            .text("model", self.config.model_name.clone().unwrap());

        let response = Client::new()
            .post(self.config.model_path.clone().unwrap())
            .header("Content-Type", "multipart/form-data")
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.clone().unwrap()),
            )
            .multipart(form)
            .send()
            .await?;

        let body = response.text().await?;
        println!("{}", body);
        let json_data: Value = serde_json::from_str(&body)?;
        Ok(json_data["text"].as_str().unwrap_or_default().to_string())
    }

    async fn get_response_offline(
        &self,
        _data: Vec<u8>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        Ok("Offline response not implemented".to_string())
    }
}

mod tests {
    use super::*;
    use crate::config::Config;
    use tokio;

    #[tokio::test]
    async fn test_generate_response() {
        let config = Config::new();
        let dmod = SpeechModel {
            config: config.speech_to_text,
        };
        let audio_file_path = "/home/10346053@zte.intra/hdy/github/assistant/test/demo.wav";
        let audio_data = std::fs::read(audio_file_path).expect("Failed to read audio file");
        let r = dmod.get_response_online(audio_data).await;
        println!("{:?}", r);
        assert!(r.is_ok());
    }
}
