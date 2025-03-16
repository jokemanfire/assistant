use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use protos::grpc::model::{MessageType, ServerMessage, StreamingRequest};
use protos::grpc::model::{
    model_service_client::ModelServiceClient, ChatMessage, Role, SpeechRequest, TextRequest,
};
use protos::grpc::mserver::{
    server_service_client::ServerServiceClient, Empty, ModelListResponse, ModelStatusRequest,
};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tonic::transport::Channel;
use url::Url;
use uuid::Uuid;

/// Assistant client for interacting with the gRPC service
pub struct AssistantClient {
    server_client: ServerServiceClient<Channel>,
    model_client: ModelServiceClient<Channel>,
}

impl AssistantClient {
    /// Create a new client connected to the specified endpoint
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())?
            .connect()
            .await?;

        let server_client = ServerServiceClient::new(channel.clone());
        let model_client = ModelServiceClient::new(channel);

        Ok(Self {
            server_client,
            model_client,
        })
    }

    /// Query available models from the server
    pub async fn query_models(&mut self) -> Result<ModelListResponse> {
        let response = self.server_client.query_models(Empty {}).await?;
        Ok(response.into_inner())
    }

    /// Query the status of a specific model
    pub async fn query_model_status(&mut self, model_id: &str) -> Result<String> {
        let request = ModelStatusRequest {
            model_id: model_id.to_string(),
        };

        let response = self.server_client.query_model_status(request).await?;
        let status = response.into_inner();

        // Convert status code to string
        let status_str = match status.status {
            0 => "Unknown",
            1 => "Loading",
            2 => "Ready",
            3 => "Error",
            _ => "Invalid status",
        };

        if !status.error.is_empty() {
            Ok(format!("{} (Error: {})", status_str, status.error))
        } else {
            Ok(status_str.to_string())
        }
    }

    /// Send a chat message to the model and get a response
    pub async fn chat(&mut self, messages: Vec<ChatMessage>) -> Result<String> {
        let request = TextRequest { messages };
        let response = self.model_client.text_chat(request).await?;
        Ok(response.into_inner().text)
    }

    /// Chat with streaming response - returns WebSocket URL
    pub async fn get_streaming_url(&mut self, messages: Vec<ChatMessage>) -> Result<String> {
        let request = StreamingRequest {
            messages,
            session_id: Uuid::new_v4().to_string(),
        };
        let response = self.model_client.streaming_text_chat(request).await?;
        Ok(response.into_inner().streaming_url)
    }

    /// Chat with streaming response - connects to WebSocket and returns a channel with responses
    pub async fn chat_stream(
        &mut self,
        messages: Vec<ChatMessage>,
    ) -> Result<mpsc::Receiver<String>> {
        let ws_url = self.get_streaming_url(messages).await?;
        let (tx, rx) = mpsc::channel(32);

        let url = Url::parse(&ws_url).map_err(|e| anyhow!("Invalid WebSocket URL: {}", e))?;

        // Connect to WebSocket and handle messages
        tokio::spawn(async move {
            match connect_async(url).await {
                Ok((ws_stream, _)) => {
                    let (_, mut read) = ws_stream.split();

                    while let Some(message) = read.next().await {
                        match message {
                            Ok(Message::Text(text)) => {
                                // println!("Received message: {}", text);
                                // Analyze the message
                                match serde_json::from_str::<ServerMessage>(&text) {
                                    Ok(server_message) => {
                                        let message_type = server_message.msg_type;
                                        if message_type == MessageType::Stream as i32 {
                                            if let Some(content) = server_message.content {
                                                if tx.send(content).await.is_err() {
                                                    break;
                                                }
                                            }
                                        } else if message_type == MessageType::Done as i32 {
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(format!("Error: {}", e)).await;
                                        break;
                                    }
                                }
                            }
                            Ok(Message::Close(_)) => {
                                break;
                            }
                            Err(e) => {
                                let _ = tx.send(format!("Error: {}", e)).await;
                                break;
                            }
                            _ => {} 
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(format!("Failed to connect to WebSocket: {}", e))
                        .await;
                }
            }
        });

        Ok(rx)
    }

    /// Helper function to create a user message
    pub fn create_user_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: Role::User as i32,
            content: content.to_string(),
        }
    }

    /// Helper function to create a system message
    pub fn create_system_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: Role::System as i32,
            content: content.to_string(),
        }
    }

    /// Helper function to create an assistant message
    pub fn create_assistant_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: Role::Assistant as i32,
            content: content.to_string(),
        }
    }

    /// Convert speech to text (not fully implemented in the server yet)
    pub async fn speech_to_text(&mut self, audio_data: &str, audio_format: &str) -> Result<String> {
        let request = SpeechRequest {
            audio_data: audio_data.to_string(),
            audio_format: audio_format.to_string(),
        };

        let response = self.model_client.speech_to_text(request).await?;
        Ok(response.into_inner().text)
    }
}
