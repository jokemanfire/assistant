pub mod config;
pub mod error;
pub mod http;
pub mod openai;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin name
    fn name(&self) -> &str;

    /// Initialize plugin
    async fn init(&mut self) -> Result<()>;

    /// Start plugin
    async fn start(&self) -> Result<()>;

    /// Stop plugin
    async fn stop(&self) -> Result<()>;
}

/// Plugin manager
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) {
        self.plugins.push(Box::new(plugin));
    }

    pub async fn init_all(&mut self) -> Result<()> {
        for plugin in &mut self.plugins {
            plugin.init().await?;
        }
        Ok(())
    }

    pub async fn start_all(&self) -> Result<()> {
        for plugin in &self.plugins {
            plugin.start().await?;
        }
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<()> {
        for plugin in &self.plugins {
            plugin.stop().await?;
        }
        Ok(())
    }
}
