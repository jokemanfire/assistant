use crate::config::{ChatConfig, ModelConfig};
use protos::grpc::model::{ChatMessage, Role};
use reqwest::Client;
use serde_json::{json, Value};

pub struct DialogueModel {
    pub config: ChatConfig,
}

impl DialogueModel {
    pub async fn get_response_online(
        &self,
        inputdata: Vec<ChatMessage>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let client = Client::new();
        let remote_conf = self.config.remote_models.clone();

        let av_remote_conf = remote_conf
            .iter()
            .filter(|model| model.enabled)
            .collect::<Vec<_>>();
        if av_remote_conf.is_empty() {
            return Err("No enabled remote models".into());
        }

        let mut response_text = String::new();
        let mut error_occurred = false;
        let mut error_message = String::new();

        // try each model until success or all fail
        for model in av_remote_conf {
   
            let result: Result<String, Box<dyn std::error::Error>> = async {
                // Convert ChatMessage to JSON array for API request
                let messages = inputdata
                    .iter()
                    .map(|msg| {
                        let role_str = match msg.role {
                            r if r == Role::User as i32 => "user",
                            r if r == Role::Assistant as i32 => "assistant",
                            r if r == Role::System as i32 => "system",
                            _ => "user", // Default to user if unknown
                        };

                        json!({
                            "role": role_str,
                            "content": &msg.content
                        })
                    })
                    .collect::<Vec<_>>();

                // If no system message is present, add a default one
                let has_system_message = inputdata.iter().any(|msg| msg.role == Role::System as i32);
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
                    "model": model.model_name.clone(),
                    "messages": final_messages
                });

                let response = client
                    .post(model.model_path.clone())
                    .header("Content-Type", "application/json")
                    .header(
                        "Authorization",
                        format!("Bearer {}", model.api_key.clone()),
                    )
                    .json(&request_json)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    return Err(
                        format!("Request failed with status code: {}", response.status()).into(),
                    );
                }

                let body = response.text().await?;
                let json_data: Value = serde_json::from_str(&body)?;
                println!("debug : '{:?}' \n ---", json_data);
                
                Ok(json_data["choices"][0]["message"]["content"]
                    .as_str()
                    .ok_or("Failed to extract response text")?
                    .to_string())
            }.await;

            match result {
                Ok(response) => {
                    response_text = response;
                    break;
                }
                Err(e) => {
                    error_occurred = true;
                    error_message = e.to_string();
                }
            }
        }

        if error_occurred && response_text.is_empty() {
            Err(error_message.into())
        } else {
            Ok(response_text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tokio;

    #[tokio::test]
    async fn test_generate_response() {
        let config = Config::new();
        let dmod = DialogueModel {
            config: config.chat_model,
        };

        // Create test messages for conversation
        let messages = vec![ChatMessage {
            role: Role::User.into(),
            content: "Hello, how are you?".to_string(),
            ..Default::default()
        }];

        let r = dmod.get_response_online(messages).await;
        println!("{:?}", r);
        assert!(r.is_ok());
    }
}
