pub mod manager;
pub mod wasm_runner;

use crate::local::manager::ModelRequest;
use anyhow::{anyhow, Result};
use async_trait::async_trait;

use tokio::sync::mpsc::Sender;

/// LocalRunner is a trait for local model runners
// Define the LocalRunner trait
#[async_trait]
pub trait LocalRunner: Send + Sync {
    /// Process a request and return the response text
    async fn deal_request(&self, request: ModelRequest) -> Result<String>;

    /// Process a streaming request and send chunks to the provided sender
    async fn deal_stream_request(
        &self,
        request: ModelRequest,
        sender: &Sender<String>,
    ) -> Result<()>;

    /// Start the runner
    async fn run(&mut self) -> Result<()>;

    /// Check if the runner is ready to process requests
    fn is_ready(&self) -> bool;
}
