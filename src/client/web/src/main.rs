use warp::Filter;
use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Deserialize)]
struct Config {
    model: String,
}

#[derive(Serialize)]
struct Response {
    text: String,
}

#[tokio::main]
async fn main() {
    let client = Client::new();
    let config_page = warp::path("config")
        .map(|| {
            warp::reply::html(r#"
                <html>
                <head>
                    <title>配置页面</title>
                    <style>
                        body { font-family: Arial, sans-serif; }
                        form { margin: 20px; }
                        input[type="text"] { width: 300px; }
                        input[type="submit"] { margin-top: 10px; }
                    </style>
                </head>
                <body>
                    <h1>模型配置</h1>
                    <form action="/dialogue" method="post">
                        <label for="model">模型名称:</label>
                        <input type="text" id="model" name="model" value="FunAudioLLM/SenseVoiceSmall" required>
                        <input type="submit" value="提交">
                    </form>
                </body>
                </html>
            "#)
        });

    // 处理对话请求
    let dialogue_route = warp::path("dialogue")
        .and(warp::post())
        .and(warp::body::form())
        .map(move |form: Config| {
            let client = client.clone();
            tokio::spawn(async move {
                let response = client.post("https://api.siliconflow.cn/v1/audio/transcriptions")
                    .header("Authorization", "Bearer <your_api_key>")
                    .header("Content-Type", "multipart/form-data")
                    .form(&[("model", form.model)])
                    .send()
                    .await;

                match response {
                    Ok(resp) => {
                        // let json: Response = resp.json().await.unwrap();
                        format!("模型返回 test")
                    }
                    Err(_) => "请求失败".to_string(),
                }
            });
            "请求已发送".to_string()
        });

    // 启动服务器
    let routes = config_page.or(dialogue_route);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}