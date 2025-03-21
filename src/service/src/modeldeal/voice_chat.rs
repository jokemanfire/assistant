use reqwest::Client;
use serde_json::Value;

use crate::config::VoiceChatConfig;

pub struct SpeechModel {
    pub config: VoiceChatConfig,
}

impl SpeechModel {
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
}

mod tests {
    use super::*;
    use crate::config::Config;
    use tokio;

    #[tokio::test]
    async fn test_generate_response() {
        let config = Config::new();
        let dmod = SpeechModel {
            config: config.voice_chat,
        };
        let audio_file_path = "/home/10346053@zte.intra/hdy/github/assistant/test/demo.wav";
        let audio_data = std::fs::read(audio_file_path).expect("Failed to read audio file");
        let r = dmod.get_response_online(audio_data).await;
        println!("{:?}", r);
        assert!(r.is_ok());
    }
}
