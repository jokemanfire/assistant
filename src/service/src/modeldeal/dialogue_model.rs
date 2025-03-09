use crate::config::DialogueModelConfig;
use crate::service::ttrpcservice::ModelDeal;
use async_trait::async_trait;
use protos::ttrpc::model::{ChatMessage, Role};
use reqwest::Client;
use serde_json::{json, Value};

pub struct DialogueModel {
    pub config: DialogueModelConfig,
}

#[async_trait]
impl ModelDeal<Vec<protos::ttrpc::model::ChatMessage>, String> for DialogueModel {
    async fn get_response_online(
        &self,
        inputdata: Vec<protos::ttrpc::model::ChatMessage>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let client = Client::new();
        
        // Convert ChatMessage to JSON array for API request
        let messages = inputdata.iter().map(|msg| {
            let role_str = match msg.role {
                r if r == Role::ROLE_USER.into() => "user",
                r if r == Role::ROLE_ASSISTANT.into() => "assistant",
                r if r == Role::ROLE_SYSTEM.into() => "system",
                _ => "user", // Default to user if unknown
            };
            
            json!({
                "role": role_str,
                "content": &msg.content
            })
        }).collect::<Vec<_>>();
        
        // If no system message is present, add a default one
        let has_system_message = inputdata.iter().any(|msg| msg.role == Role::ROLE_SYSTEM.into());
        let final_messages = if !has_system_message {
            let mut msgs = vec![json!({
                "role": "system",
                "content": "you are a helpful assistant"
            })];
            msgs.extend(messages);
            msgs
        } else {
            messages
        };

        let request_json = json!({
            "model": self.config.remote_models[0].model_name.clone(),
            "messages": final_messages
        });
        
        let response = client
            .post(self.config.remote_models[0].model_path.clone())
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.config.remote_models[0].api_key.clone()),
            )
            .json(&request_json)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_generate_response() {
        let config = Config::new();
        let dmod = DialogueModel {
            config: config.dialogue_model,
        };
        
        // Create test messages for conversation
        let messages = vec![
            ChatMessage {
                role: Role::ROLE_USER.into(),
                content: "Hello, how are you?".to_string(),
                ..Default::default()
            }
        ];
        
        let r = dmod.get_response_online(messages).await;
        println!("{:?}", r);
        assert!(r.is_ok());
    }
}
