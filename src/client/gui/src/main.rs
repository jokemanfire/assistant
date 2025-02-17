use eframe::egui::{FontFamily, FontId, StrokeKind};
use eframe::{egui, App};
use egui::{Color32, RichText, ScrollArea, TextStyle};
use serde::{Deserialize, Serialize};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

use api::dialogue_model::dialogue_model;

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
    timestamp: i64,
}

#[derive(Serialize, Deserialize)]
struct Conversation {
    id: String,
    messages: Vec<Message>,
}

struct ChatApp {
    input_text: String,
    conversations: Arc<Mutex<Vec<Conversation>>>,
    current_conversation_id: String,
    is_sending: Arc<Mutex<bool>>,
    runtime: Runtime,
}

impl Default for ChatApp {
    fn default() -> Self {
        let runtime = Runtime::new().unwrap();
        // 创建初始对话
        let initial_conversation = Conversation {
            id: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string(),
            messages: vec![Message {
                role: "system".to_string(),
                content: "welcome to use AI chat system".to_string(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            }],
        };

        Self {
            input_text: String::new(),
            current_conversation_id: initial_conversation.id.clone(),
            conversations: Arc::new(Mutex::new(vec![initial_conversation])),
            is_sending: Arc::new(Mutex::new(false)),
            runtime,
        }
    }
}

impl App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.visuals.window_fill = Color32::from_rgb(255, 255, 255);

        ctx.set_style(style);

        egui::SidePanel::left("conversations_panel")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading(RichText::new("Chat list").size(20.0));
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(8.0);

                if ui
                    .add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(
                            RichText::new("+ Create Chat")
                                .color(Color32::from_rgb(57, 197, 187))
                                .size(16.0),
                        ),
                    )
                    .clicked()
                {
                    let new_conversation = Conversation {
                        id: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                            .to_string(),
                        messages: vec![],
                    };
                    self.current_conversation_id = new_conversation.id.clone();
                    self.conversations.lock().unwrap().push(new_conversation);
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ScrollArea::vertical().show(ui, |ui| {
                    for conversation in self.conversations.lock().unwrap().iter() {
                        let is_current = conversation.id == self.current_conversation_id;
                        let text = if is_current {
                            RichText::new(format!("💬 chat {}", &conversation.id[..8]))
                                .color(Color32::from_rgb(57, 197, 187))
                                .size(14.0)
                        } else {
                            RichText::new(format!("💬 chat {}", &conversation.id[..8]))
                                .color(Color32::GRAY)
                                .size(14.0)
                        };

                        let response =
                            ui.add_sized([ui.available_width(), 32.0], egui::Button::new(text));

                        if response.clicked() {
                            self.current_conversation_id = conversation.id.clone();
                        }

                        if is_current {
                            response.highlight();
                        }
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("AI Chat system");
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(16.0);

            // 消息显示区域
            let available_height = ui.available_height() - 120.0;
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .max_height(available_height)
                .show(ui, |ui| {
                    let conversations = self.conversations.lock().unwrap();
                    if let Some(conversation) = conversations
                        .iter()
                        .find(|c| c.id == self.current_conversation_id)
                    {
                        for message in &conversation.messages {
                            let is_user = message.role == "user";

                            ui.add_space(8.0);
                            let mut layout = if is_user {
                                egui::Layout::right_to_left(egui::Align::TOP)
                            } else {
                                egui::Layout::left_to_right(egui::Align::TOP)
                            };
                            layout.main_justify = true;

                            ui.with_layout(layout, |ui| {
                                let max_width = ui.available_width() * 0.8;

                                // 先创建文本标签但不立即显示
                                let text = if is_user {
                                    RichText::new(format!("{}  👤", message.content))
                                        .color(Color32::WHITE)
                                        .size(15.0)
                                } else {
                                    RichText::new(format!("AI\n  {}", message.content))
                                        .color(if is_user {
                                            Color32::WHITE
                                        } else {
                                            Color32::BLACK
                                        })
                                        .size(15.0)
                                };

                                let galley = ui.painter().layout_no_wrap(
                                    text.text().into(),
                                    FontId::new(15.0, FontFamily::Proportional),
                                    Color32::BLACK,
                                );

                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(max_width, galley.size().y + 16.0),
                                    egui::Sense::hover(),
                                );

                                // 先绘制背景
                                let bg_rect = rect.expand(8.0);
                                let bg_color = if is_user {
                                    Color32::from_rgb(57, 197, 187)
                                } else {
                                    Color32::from_rgb(240, 240, 240)
                                };

                                ui.painter().rect(
                                    bg_rect,
                                    10.0,
                                    bg_color,
                                    egui::Stroke::new(0.0, bg_color),
                                    egui::StrokeKind::Outside,
                                );

                                // 然后绘制文本
                                ui.painter().galley(
                                    rect.min + egui::vec2(8.0, 8.0),
                                    galley,
                                    Color32::BLACK,
                                );
                            });
                            ui.add_space(8.0);
                        }
                    }
                });
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);
            // 输入区域
            egui::TopBottomPanel::bottom("input_panel")
                .min_height(100.0)
                .show_inside(ui, |ui| {
                    ui.add_space(10.0);

                    // 输入区域
                    ui.horizontal(|ui| {
                        let input_text_edit = egui::TextEdit::multiline(&mut self.input_text)
                            .desired_rows(3)
                            .desired_width(ui.available_width() - 100.0)
                            .hint_text("Input message...")
                            .font(egui::TextStyle::Body);

                        let response = ui.add(input_text_edit);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            let send_button =
                                egui::Button::new(RichText::new("send").size(16.0).color(
                                    if !*self.is_sending.lock().unwrap() {
                                        Color32::BLACK
                                    } else {
                                        Color32::GRAY
                                    },
                                ))
                                .min_size(egui::vec2(80.0, 30.0))
                                .fill(
                                    if !*self.is_sending.lock().unwrap() {
                                        Color32::LIGHT_GRAY
                                    } else {
                                        Color32::from_rgb(57, 197, 187)
                                    },
                                );

                            if ui
                                .add_enabled(!*self.is_sending.lock().unwrap(), send_button)
                                .clicked()
                                || (response.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                if !self.input_text.is_empty() {
                                    self.send_message();
                                }
                            }
                        });
                    });

                    if *self.is_sending.lock().unwrap().deref_mut() {
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            ui.label("Sending...");
                        });
                    }

                    ui.add_space(8.0);
                });
        });

        ctx.request_repaint();
    }
}

impl ChatApp {
    fn send_message(&mut self) {
        if let Some(conversation) = self
            .conversations
            .lock()
            .unwrap()
            .iter_mut()
            .find(|c| c.id == self.current_conversation_id)
        {
            let user_message = Message {
                role: "user".to_string(),
                content: self.input_text.clone(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            };

            conversation.messages.push(user_message);

            // 创建用于发送的消息
            let message_to_send = self.input_text.clone();
            self.input_text.clear();
            *self.is_sending.lock().unwrap() = true;

            // 克隆必要的数据用于异步闭包
            let conversation_id = self.current_conversation_id.clone();
            let conversations = Arc::clone(&self.conversations);
            let is_sending = Arc::clone(&self.is_sending);
            // 使用 tokio 运行异步任务
            self.runtime.spawn(async move {
                match dialogue_model(message_to_send).await {
                    Ok(response) => {
                        let mut conversations = conversations.lock().unwrap();
                        if let Some(conversation) =
                            conversations.iter_mut().find(|c| c.id == conversation_id)
                        {
                            conversation.messages.push(Message {
                                role: "assistant".to_string(),
                                content: response,
                                timestamp: SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs() as i64,
                            });
                        }
                        *is_sending.lock().unwrap() = false;
                    }
                    Err(e) => {
                        eprintln!("Error sending message: {:?}", e);
                    }
                }
            });
        }
    }
}

fn main() {
    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "AI Assistant",
        native_options,
        Box::new(|_cc| Ok(Box::new(ChatApp::default()))),
    );
}
