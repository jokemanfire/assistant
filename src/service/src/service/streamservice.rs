use crate::config::Config;
use crate::modeldeal::chat::DialogueModel;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use protos::grpc::model::{ChatMessage, Role};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use uuid::Uuid;

// Constants
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_CONNECTIONS_PER_SESSION: usize = 5;

// Session state
#[derive(Debug, Clone)]
struct Session {
    id: String,
    messages: Vec<ChatMessage>,
    created_at: std::time::Instant,
    last_active: std::time::Instant,
}

// WebSocket message types
#[derive(Serialize, Deserialize, Debug)]
struct ClientMessage {
    #[serde(rename = "type")]
    msg_type: String,
    content: Option<String>,
    role: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ServerMessage {
    #[serde(rename = "type")]
    msg_type: String,
    content: Option<String>,
    session_id: Option<String>,
    error: Option<String>,
}

// Connection tracking
type Sessions = Arc<Mutex<HashMap<String, Session>>>;
type Connections = Arc<Mutex<HashMap<String, Vec<mpsc::UnboundedSender<String>>>>>;

pub struct StreamService {
    config: Config,
    sessions: Sessions,
    connections: Connections,
    dialogue_model: Arc<DialogueModel>,
    local_service: Arc<crate::service::localservice::LocalService>,
}

impl StreamService {
    pub async fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let local_service = Arc::new(
            crate::service::localservice::LocalService::new(config.chat_model.local_models.clone())
                .await,
        );

        Ok(Self {
            dialogue_model: Arc::new(DialogueModel {
                config: config.chat_model.clone(),
            }),
            local_service,
            config,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    // Generate a WebSocket URL for a new session
    pub fn generate_ws_url(
        &self,
        messages: Vec<ChatMessage>,
        session_id: Option<String>,
    ) -> String {
        let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());

        // Store the session
        let now = std::time::Instant::now();
        let session = Session {
            id: session_id.clone(),
            messages,
            created_at: now,
            last_active: now,
        };

        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(session_id.clone(), session);
        }

        // Generate WebSocket URL
        let ws_host = self.config.server.ws_addr.clone().unwrap_or_else(|| {
            if let Some(grpc_addr) = &self.config.server.grpc_addr {
                // Extract host from gRPC address
                grpc_addr
                    .split(':')
                    .next()
                    .unwrap_or("localhost")
                    .to_string()
                    + ":8080"
            } else {
                "localhost:8080".to_string()
            }
        });

        format!("ws://{}/chat/{}", ws_host, session_id)
    }

    // Start the WebSocket server
    pub async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ws_addr = self
            .config
            .server
            .ws_addr
            .clone()
            .unwrap_or_else(|| "127.0.0.1:8080".to_string());
        let addr: SocketAddr = ws_addr.parse()?;

        // Create TCP listener
        let listener = TcpListener::bind(&addr).await?;
        info!("WebSocket server started on {}", addr);

        // Clone shared state
        let sessions = self.sessions.clone();
        let connections = self.connections.clone();
        let dialogue_model = self.dialogue_model.clone();
        let local_service = self.local_service.clone();

        // Start cleanup task
        let cleanup_sessions = self.sessions.clone();
        let cleanup_connections = self.connections.clone();
        tokio::spawn(async move {
            loop {
                Self::cleanup_stale_sessions(cleanup_sessions.clone(), cleanup_connections.clone())
                    .await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        // Accept connections
        while let Ok((stream, _)) = listener.accept().await {
            let peer = stream.peer_addr()?;
            info!("Peer connected: {}", peer);

            // Clone shared state for this connection
            let sessions_clone = sessions.clone();
            let connections_clone = connections.clone();
            let dialogue_model_clone = dialogue_model.clone();
            let local_service_clone = local_service.clone();

            // Spawn a task to handle the connection
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    stream,
                    peer,
                    sessions_clone,
                    connections_clone,
                    dialogue_model_clone,
                    local_service_clone,
                )
                .await
                {
                    error!("Error processing connection: {}", e);
                }
            });
        }

        Ok(())
    }

    // Start the WebSocket server in the background
    pub async fn start_in_background(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ws_addr = self
            .config
            .server
            .ws_addr
            .clone()
            .unwrap_or_else(|| "127.0.0.1:8080".to_string());
        let addr: SocketAddr = ws_addr.parse()?;

        // Clone shared state
        let sessions = self.sessions.clone();
        let connections = self.connections.clone();
        let dialogue_model = self.dialogue_model.clone();
        let local_service = self.local_service.clone();

        info!("Starting WebSocket server in background on {}", addr);

        // Start cleanup task
        let cleanup_sessions = self.sessions.clone();
        let cleanup_connections = self.connections.clone();
        tokio::spawn(async move {
            loop {
                Self::cleanup_stale_sessions(cleanup_sessions.clone(), cleanup_connections.clone())
                    .await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        // Start server in background
        tokio::spawn(async move {
            // Create TCP listener
            match TcpListener::bind(&addr).await {
                Ok(listener) => {
                    info!("WebSocket server started on {}", addr);

                    // Accept connections
                    while let Ok((stream, _)) = listener.accept().await {
                        if let Ok(peer) = stream.peer_addr() {
                            info!("Peer connected: {}", peer);

                            // Clone shared state for this connection
                            let sessions_clone = sessions.clone();
                            let connections_clone = connections.clone();
                            let dialogue_model_clone = dialogue_model.clone();
                            let local_service_clone = local_service.clone();

                            // Spawn a task to handle the connection
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(
                                    stream,
                                    peer,
                                    sessions_clone,
                                    connections_clone,
                                    dialogue_model_clone,
                                    local_service_clone,
                                )
                                .await
                                {
                                    error!("Error processing connection: {}", e);
                                }
                            });
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to bind WebSocket server: {}", e);
                }
            }
        });

        // Wait a bit to ensure server starts
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(())
    }

    // Handle a WebSocket connection
    async fn handle_connection(
        stream: TcpStream,
        peer: SocketAddr,
        sessions: Sessions,
        connections: Connections,
        dialogue_model: Arc<DialogueModel>,
        local_service: Arc<crate::service::localservice::LocalService>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Accept the WebSocket connection
        let ws_stream = accept_async(stream).await?;
        info!("WebSocket connection established with: {}", peer);

        // Extract path from the request to get session ID
        // Note: In a real implementation, you would extract this from the HTTP request
        // For now, we'll assume the session ID is in the URL path
        let path = peer.to_string(); // Placeholder
        let parts: Vec<&str> = path.split('/').collect();
        let session_id = if parts.len() > 2 && parts[1] == "chat" {
            parts[2].to_string()
        } else {
            debug!("Invalid WebSocket path: {}", path);
            return Err("Invalid WebSocket path".into());
        };

        // Check if session exists
        let session = {
            let mut sessions_lock = sessions.lock().unwrap();
            if let Some(session) = sessions_lock.get_mut(&session_id) {
                session.last_active = std::time::Instant::now();
                session.clone()
            } else {
                // Session not found
                return Err("Session not found".into());
            }
        };

        // Check connection limit
        {
            let mut connections_lock = connections.lock().unwrap();
            let session_connections = connections_lock
                .entry(session_id.clone())
                .or_insert_with(Vec::new);
            if session_connections.len() >= MAX_CONNECTIONS_PER_SESSION {
                // Too many connections
                return Err("Too many connections for this session".into());
            }
        }

        // Split the WebSocket stream
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Create a channel for sending messages to the WebSocket
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Store the sender
        {
            let mut connections_lock = connections.lock().unwrap();
            let session_connections = connections_lock
                .entry(session_id.clone())
                .or_insert_with(Vec::new);
            session_connections.push(tx.clone());
        }

        // Send welcome message
        let welcome_msg = ServerMessage {
            msg_type: "connected".to_string(),
            content: None,
            session_id: Some(session_id.clone()),
            error: None,
        };

        let welcome_json = serde_json::to_string(&welcome_msg)?;
        tx.send(welcome_json)?;

        // Forward messages from the channel to the WebSocket
        let forward_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_sender.send(Message::Text(msg)).await {
                    error!("WebSocket send error: {}", e);
                    break;
                }
            }
        });

        // Start processing the initial messages if any
        if !session.messages.is_empty() {
            let messages_clone = session.messages.clone();
            let session_id_clone = session_id.clone();
            let tx_clone = tx.clone();
            let dialogue_model_clone = dialogue_model.clone();
            let local_service_clone = local_service.clone();

            tokio::spawn(async move {
                Self::process_messages(
                    messages_clone,
                    session_id_clone,
                    tx_clone,
                    dialogue_model_clone,
                    local_service_clone,
                )
                .await;
            });
        }

        // Set up heartbeat
        let heartbeat_tx = tx.clone();
        let heartbeat_session_id = session_id.clone();
        let heartbeat_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(HEARTBEAT_INTERVAL).await;
                let heartbeat_msg = ServerMessage {
                    msg_type: "heartbeat".to_string(),
                    content: None,
                    session_id: Some(heartbeat_session_id.clone()),
                    error: None,
                };

                if heartbeat_tx
                    .send(serde_json::to_string(&heartbeat_msg).unwrap())
                    .is_err()
                {
                    break;
                }
            }
        });

        // Process incoming messages
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(msg) => {
                    if msg.is_close() {
                        break;
                    }

                    // Update session activity
                    {
                        let mut sessions_lock = sessions.lock().unwrap();
                        if let Some(session) = sessions_lock.get_mut(&session_id) {
                            session.last_active = std::time::Instant::now();
                        }
                    }

                    // Handle text message
                    if let Message::Text(text) = msg {
                        Self::handle_client_message(
                            &text,
                            &session_id,
                            tx.clone(),
                            sessions.clone(),
                            dialogue_model.clone(),
                            local_service.clone(),
                        )
                        .await;
                    }
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        // Connection closed
        info!("WebSocket connection closed for session: {}", session_id);
        heartbeat_handle.abort();
        forward_task.abort();

        // Remove connection
        {
            let mut connections_lock = connections.lock().unwrap();
            if let Some(session_connections) = connections_lock.get_mut(&session_id) {
                session_connections.retain(|conn| !conn.is_closed());
                if session_connections.is_empty() {
                    connections_lock.remove(&session_id);
                }
            }
        }

        Ok(())
    }

    // Handle client message
    async fn handle_client_message(
        text: &str,
        session_id: &str,
        tx: mpsc::UnboundedSender<String>,
        sessions: Sessions,
        dialogue_model: Arc<DialogueModel>,
        local_service: Arc<crate::service::localservice::LocalService>,
    ) {
        // Parse client message
        let client_msg: ClientMessage = match serde_json::from_str(text) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Failed to parse client message: {}", e);
                let error_msg = ServerMessage {
                    msg_type: "error".to_string(),
                    content: None,
                    session_id: Some(session_id.to_string()),
                    error: Some("Invalid message format".to_string()),
                };
                let _ = tx.send(serde_json::to_string(&error_msg).unwrap());
                return;
            }
        };

        match client_msg.msg_type.as_str() {
            "message" => {
                // Handle chat message
                if let Some(content) = client_msg.content {
                    let role = match client_msg.role.as_deref() {
                        Some("system") => Role::System,
                        Some("assistant") => Role::Assistant,
                        _ => Role::User, // Default to user
                    };

                    // Create chat message
                    let chat_msg = ChatMessage {
                        role: role as i32,
                        content,
                    };

                    // Update session with new message
                    let messages = {
                        let mut sessions_lock = sessions.lock().unwrap();
                        if let Some(session) = sessions_lock.get_mut(session_id) {
                            session.messages.push(chat_msg);
                            session.last_active = std::time::Instant::now();
                            session.messages.clone()
                        } else {
                            // Session not found
                            let error_msg = ServerMessage {
                                msg_type: "error".to_string(),
                                content: None,
                                session_id: Some(session_id.to_string()),
                                error: Some("Session not found".to_string()),
                            };
                            let _ = tx.send(serde_json::to_string(&error_msg).unwrap());
                            return;
                        }
                    };

                    // Process messages
                    tokio::spawn(Self::process_messages(
                        messages,
                        session_id.to_string(),
                        tx,
                        dialogue_model,
                        local_service,
                    ));
                }
            }
            "ping" => {
                // Respond to ping
                let pong_msg = ServerMessage {
                    msg_type: "pong".to_string(),
                    content: None,
                    session_id: Some(session_id.to_string()),
                    error: None,
                };
                let _ = tx.send(serde_json::to_string(&pong_msg).unwrap());
            }
            _ => {
                // Unknown message type
                let error_msg = ServerMessage {
                    msg_type: "error".to_string(),
                    content: None,
                    session_id: Some(session_id.to_string()),
                    error: Some("Unknown message type".to_string()),
                };
                let _ = tx.send(serde_json::to_string(&error_msg).unwrap());
            }
        }
    }

    // Process messages and generate response
    async fn process_messages(
        messages: Vec<ChatMessage>,
        session_id: String,
        tx: mpsc::UnboundedSender<String>,
        dialogue_model: Arc<DialogueModel>,
        local_service: Arc<crate::service::localservice::LocalService>,
    ) {
        // Send "thinking" message
        let thinking_msg = ServerMessage {
            msg_type: "thinking".to_string(),
            content: None,
            session_id: Some(session_id.clone()),
            error: None,
        };
        let _ = tx.send(serde_json::to_string(&thinking_msg).unwrap());

        // Try remote model first if configured
        if !dialogue_model.config.remote_models.is_empty() {
            match dialogue_model
                .get_streaming_response_online(messages.clone(), tx.clone(), session_id.clone())
                .await
            {
                Ok(_) => return, // Successfully processed by remote model
                Err(e) => warn!("Remote model streaming failed: {}", e),
            }
        }

        // Try local model
        match tokio::time::timeout(
            Duration::from_secs(1),
            // Check for available runners
            {
                let local_service = local_service.clone();
                tokio::spawn(async move {
                    loop {
                        let available_runners = local_service.available_runners().await;
                        if available_runners > 0 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
            },
        )
        .await
        {
            Ok(_) => {
                // Local model is available
                match local_service
                    .stream_chat(messages, tx.clone(), session_id.clone())
                    .await
                {
                    Ok(_) => return, // Successfully processed by local model
                    Err(e) => {
                        warn!("Local model streaming failed: {}", e);
                        let error_msg = ServerMessage {
                            msg_type: "error".to_string(),
                            content: None,
                            session_id: Some(session_id),
                            error: Some(format!("Processing failed: {}", e)),
                        };
                        let _ = tx.send(serde_json::to_string(&error_msg).unwrap());
                    }
                }
            }
            Err(_) => {
                // No local model available
                let error_msg = ServerMessage {
                    msg_type: "error".to_string(),
                    content: None,
                    session_id: Some(session_id),
                    error: Some("No model available for processing".to_string()),
                };
                let _ = tx.send(serde_json::to_string(&error_msg).unwrap());
            }
        }
    }

    // Clean up stale sessions
    async fn cleanup_stale_sessions(sessions: Sessions, connections: Connections) {
        let now = std::time::Instant::now();
        let mut sessions_to_remove = Vec::new();

        // Find stale sessions
        {
            let sessions_lock = sessions.lock().unwrap();
            for (id, session) in sessions_lock.iter() {
                if now.duration_since(session.last_active) > CLIENT_TIMEOUT {
                    sessions_to_remove.push(id.clone());
                }
            }
        }

        // Remove stale sessions and connections
        if !sessions_to_remove.is_empty() {
            let mut sessions_lock = sessions.lock().unwrap();
            let mut connections_lock = connections.lock().unwrap();

            for id in sessions_to_remove {
                sessions_lock.remove(&id);
                connections_lock.remove(&id);
                debug!("Removed stale session: {}", id);
            }
        }
    }
}

// Extension methods for DialogueModel
impl DialogueModel {
    // Get streaming response from remote model
    pub async fn get_streaming_response_online(
        &self,
        messages: Vec<ChatMessage>,
        tx: mpsc::UnboundedSender<String>,
        session_id: String,
    ) -> Result<(), Box<dyn Error>> {
        // Implementation would depend on how remote models handle streaming
        // This is a placeholder that simulates streaming by sending chunks

        // In a real implementation, you would connect to your remote model API
        // that supports streaming and forward the chunks to the WebSocket

        // Simulate processing delay
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Get a full response first (this would be replaced with actual streaming)
        let full_response = self.get_response_online(messages).await?;

        // Simulate streaming by sending chunks
        let chunks = Self::split_into_chunks(&full_response, 10);
        for chunk in chunks {
            let stream_msg = ServerMessage {
                msg_type: "stream".to_string(),
                content: Some(chunk),
                session_id: Some(session_id.clone()),
                error: None,
            };

            tx.send(serde_json::to_string(&stream_msg).unwrap())?;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Send completion message
        let done_msg = ServerMessage {
            msg_type: "done".to_string(),
            content: Some(full_response),
            session_id: Some(session_id),
            error: None,
        };

        tx.send(serde_json::to_string(&done_msg).unwrap())?;

        Ok(())
    }

    // Helper to split text into chunks
    fn split_into_chunks(text: &str, chunk_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();

        for chunk in chars.chunks(chunk_size) {
            chunks.push(chunk.iter().collect());
        }

        chunks
    }
}

// Extension methods for LocalService
impl crate::service::localservice::LocalService {
    // Stream chat response
    pub async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        tx: mpsc::UnboundedSender<String>,
        session_id: String,
    ) -> Result<(), Box<dyn Error>> {
        // In a real implementation, you would connect to your local model
        // and stream the response chunks

        // For now, we'll simulate streaming with the regular chat method
        match self.chat_stream(messages).await {
            Ok(mut response_channel) => {
                // Read from the channel
                while let Some(response) = response_channel.recv().await {
                    let chunks = DialogueModel::split_into_chunks(&response, 10);
                    for chunk in chunks {
                        let stream_msg = ServerMessage {
                            msg_type: "stream".to_string(),
                            content: Some(chunk),
                            session_id: Some(session_id.clone()),
                            error: None,
                        };

                        tx.send(serde_json::to_string(&stream_msg).unwrap())?;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }

                    // Send completion message
                    let done_msg = ServerMessage {
                        msg_type: "done".to_string(),
                        content: Some(response),
                        session_id: Some(session_id.clone()),
                        error: None,
                    };

                    tx.send(serde_json::to_string(&done_msg).unwrap())?;
                }
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}
