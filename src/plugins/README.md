# Assistant Plugins

这个包提供了Assistant服务的插件系统，目前支持以下插件：

## OpenAI兼容API

这个插件提供了与OpenAI API兼容的HTTP接口，允许使用OpenAI客户端库或工具与Assistant服务进行交互。

### 支持的接口

- `/v1/chat/completions` - 聊天完成API
- `/v1/completions` - 文本完成API

### 使用方法

1. 在编译时启用`http_api`特性：

```bash
cargo build --features http_api
```

2. 使用OpenAI客户端库或工具发送请求：

```bash
curl https://api.openai.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is the capital of France?"}
    ],
    "temperature": 0.7
  }'
```

### 配置

插件的配置在`PluginsConfig`结构体中定义，包括：

- HTTP服务配置（监听地址、端口等）
- OpenAI API配置（API密钥、模型映射等）

## 扩展新的API兼容插件

要添加新的API兼容插件，可以按照以下步骤进行：

1. 在`src/plugins/src`目录下创建新的模块
2. 实现`Plugin` trait
3. 在`PluginsConfig`中添加新的配置项
4. 在`main.rs`中注册新的插件

## 示例：添加新的API兼容插件

```rust
// 1. 创建新的模块
pub mod my_api;

// 2. 实现Plugin trait
pub struct MyApiPlugin {
    name: String,
    config: MyApiConfig,
}

#[async_trait]
impl Plugin for MyApiPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn init(&mut self) -> Result<()> {
        // 初始化逻辑
        Ok(())
    }
    
    async fn start(&self) -> Result<()> {
        // 启动逻辑
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        // 停止逻辑
        Ok(())
    }
}

// 3. 在PluginsConfig中添加新的配置项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    pub http: HttpConfig,
    pub openai: OpenAIConfig,
    pub my_api: MyApiConfig,
}

// 4. 在main.rs中注册新的插件
let my_api_plugin = MyApiPlugin::new(plugins_config.my_api.clone());
manager.register(my_api_plugin);
``` 