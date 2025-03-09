use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Deserialize, Serialize, Debug)]
struct Message {
    role: String,
    content: String,
    timestamp: i64,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
struct Conversation {
    id: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct Config {
    model: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    conversation_id: String,
    message: String,
}

// 配置页面处理函数
async fn config_page() -> impl Responder {
    let html = fs::read_to_string("templates/config.html")
        .unwrap_or_else(|_| "无法加载配置页面模板".to_string());
    HttpResponse::Ok().content_type("text/html").body(html)
}

// 聊天页面处理函数
async fn chat_page(path: web::Path<String>) -> impl Responder {
    let conversation_id = path.into_inner();
    let template = fs::read_to_string("templates/chat.html")
        .unwrap_or_else(|_| "无法加载聊天页面模板".to_string());

    let html = template.replace("{{conversation_id}}", &conversation_id);
    HttpResponse::Ok().content_type("text/html").body(html)
}

// 发送消息处理函数
async fn send_message(
    data: web::Json<ChatRequest>,
    conversations: web::Data<Arc<Mutex<HashMap<String, Conversation>>>>,
) -> impl Responder {
    let mut conversations = conversations.lock().await;
    let conversation = conversations
        .entry(data.conversation_id.clone())
        .or_insert(Conversation {
            id: data.conversation_id.clone(),
            messages: Vec::new(),
        });

    // 添加用户消息
    conversation.messages.push(Message {
        role: "user".to_string(),
        content: data.message.clone(),
        timestamp: chrono::Utc::now().timestamp(),
    });

    // 将对话历史转换为 ChatMessage 格式
    let chat_messages: Vec<protos::ttrpc::model::ChatMessage> = conversation
        .messages
        .iter()
        .map(|msg| {
            // 将角色字符串转换为 Role 枚举
            let role = match msg.role.as_str() {
                "user" => protos::ttrpc::model::Role::ROLE_USER,
                "assistant" => protos::ttrpc::model::Role::ROLE_ASSISTANT,
                "system" => protos::ttrpc::model::Role::ROLE_SYSTEM,
                _ => protos::ttrpc::model::Role::ROLE_USER, // 默认为用户
            };

            // 创建 ChatMessage
            protos::ttrpc::model::ChatMessage {
                role: role.into(),
                content: msg.content.clone(),
                ..Default::default()
            }
        })
        .collect();

    // 调用 TTRPC 服务，传入完整对话历史
    match api::dialogue_model::dialogue_model(chat_messages).await {
        Ok(response) => {
            // 添加 AI 响应消息
            conversation.messages.push(Message {
                role: "assistant".to_string(),
                content: response,
                timestamp: chrono::Utc::now().timestamp(),
            });
            HttpResponse::Ok().json(&conversation.messages)
        }
        Err(e) => {
            println!("TTRPC 调用错误: {:?}", e);
            HttpResponse::InternalServerError().body("AI 服务调用失败")
        }
    }
}

// 获取历史记录处理函数
async fn get_history(
    path: web::Path<String>,
    conversations: web::Data<Arc<Mutex<HashMap<String, Conversation>>>>,
) -> impl Responder {
    let conversations = conversations.lock().await;
    if let Some(conversation) = conversations.get(&path.into_inner()) {
        HttpResponse::Ok().json(&conversation.messages)
    } else {
        HttpResponse::Ok().json(&Vec::<Message>::new())
    }
}

// 获取所有对话列表
async fn get_conversations(
    conversations: web::Data<Arc<Mutex<HashMap<String, Conversation>>>>,
) -> impl Responder {
    let conversations = conversations.lock().await;
    let mut chat_list: Vec<_> = conversations
        .values()
        .map(|conv| {
            let last_message = conv.messages.last();
            let last_update = last_message.map(|msg| msg.timestamp).unwrap_or(0);
            serde_json::json!({
                "id": conv.id,
                "lastUpdate": last_update
            })
        })
        .collect();

    // 按最后更新时间排序
    chat_list.sort_by(|a, b| {
        b["lastUpdate"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["lastUpdate"].as_i64().unwrap_or(0))
    });

    HttpResponse::Ok().json(chat_list)
}

// 清除对话历史
async fn clear_messages(
    path: web::Path<String>,
    conversations: web::Data<Arc<Mutex<HashMap<String, Conversation>>>>,
) -> impl Responder {
    let mut conversations = conversations.lock().await;
    let conversation_id = path.into_inner();

    match conversations.get_mut(&conversation_id) {
        Some(conversation) => {
            // 清空消息数组
            conversation.messages.clear();
            HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "对话已清除"
            }))
        }
        None => HttpResponse::NotFound().json(json!({
            "status": "error",
            "message": "对话不存在"
        })),
    }
}

// 删除对话
async fn delete_chat(
    path: web::Path<String>,
    conversations: web::Data<Arc<Mutex<HashMap<String, Conversation>>>>,
) -> impl Responder {
    let mut conversations = conversations.lock().await;
    let conversation_id = path.into_inner();

    if conversations.remove(&conversation_id).is_some() {
        HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "对话已删除"
        }))
    } else {
        HttpResponse::NotFound().json(json!({
            "status": "error",
            "message": "对话不存在"
        }))
    }
}

// 首页处理函数
async fn index_page() -> impl Responder {
    let html = fs::read_to_string("templates/index.html")
        .unwrap_or_else(|_| "无法加载首页模板".to_string());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let conversations: Arc<Mutex<HashMap<String, Conversation>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let conversations_data = web::Data::new(conversations);

    println!("服务器启动在 http://0.0.0.0:3030");

    HttpServer::new(move || {
        App::new()
            .app_data(conversations_data.clone())
            .route("/", web::get().to(index_page))
            .route("/config", web::get().to(config_page))
            .route("/chat/{id}", web::get().to(chat_page))
            .route("/send", web::post().to(send_message))
            .route("/history/{id}", web::get().to(get_history))
            .route("/conversations", web::get().to(get_conversations))
            .route("/clear/{id}", web::post().to(clear_messages))
            .route("/delete/{id}", web::post().to(delete_chat))
    })
    .bind("127.0.0.1:3030")?
    .run()
    .await
}
