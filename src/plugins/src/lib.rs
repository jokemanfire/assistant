pub mod config;
pub mod error;
pub mod http;
pub mod openai;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件名称
    fn name(&self) -> &str;

    /// 初始化插件
    async fn init(&mut self) -> Result<()>;

    /// 启动插件
    async fn start(&self) -> Result<()>;

    /// 停止插件
    async fn stop(&self) -> Result<()>;
}

/// 插件管理器
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
