use crate::config::{Config, DialogueModelConfig};
use crate::server::ModelDeal;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub struct DialogueModel {
    pub config:DialogueModelConfig,
}

#[async_trait]
impl ModelDeal<String, String> for DialogueModel {
    async fn get_response_online(
        &self,
        inputdata: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let client = Client::new();
        let response = client
            .post(self.config.model_path.clone().unwrap())
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.clone().unwrap()),
            )
            .json(&json!({
                "model": self.config.model_name.clone().unwrap(),
                "messages": [
                    {
                        "role": "system",
                        "content": "you are a helpful assistant"
                    },
                    {
                        "role": "user",
                        "content": inputdata
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
        println!("debug : '{:?}' \n ---", json_data);
        let response_text = json_data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("Failed to extract response text")?;
        Ok(response_text.to_string())
    }

    async fn get_response_offline(
        &self,
        inputdata: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        todo!()
    }
}

#[cfg(test)]

mod tests {
    use crate::dialogue_model;

    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_generate_response() {
        let config = Config::new();
        let dmod = DialogueModel{
            config:config.dialogue_model,
        };
        let r = dmod.get_response_online("Hello".to_string()).await;
        println!("{:?}",r);
        assert!(r.is_ok());
    }
}
