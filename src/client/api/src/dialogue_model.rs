use protos::ttrpc::{model,model_ttrpc};
use std::{error::Error, time::Duration};
use ttrpc::{
    asynchronous::Client,
    context::{self, Context},
};

const DIALOGUE_MODEL_API_URL: &str = "unix:///tmp/ttrpc-test";

fn default_ctx() -> Context {
    let mut ctx = context::with_duration(Duration::from_secs(300));
    ctx.add("key-1".to_string(), "value-1-1".to_string());
    ctx.add("key-1".to_string(), "value-1-2".to_string());
    ctx.set("key-2".to_string(), vec!["value-2".to_string()]);

    ctx
}

pub async fn dialogue_model(input_text: String) -> Result<String, Box<dyn Error>> {
    let client = Client::connect(DIALOGUE_MODEL_API_URL)?;
    let ttrpc_client = model_ttrpc::ModelServiceClient::new(client);
    let req = model::TextRequest {
        text: input_text.to_string(),
        ..Default::default()
    };
    println!("Sending text chat request: {:?}", req.text);
    let output = ttrpc_client.text_chat(default_ctx(), &req).await?;
    println!("Received text chat response: {:?}", output.text);
    Ok(output.text)
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_dialogue_model() {
        loop {
            let text = "Hello, how are you?".to_string();
            let client = Client::connect(DIALOGUE_MODEL_API_URL).unwrap();

            let ttrpc_client = model_ttrpc::ModelServiceClient::new(client);
            let req = model::TextRequest {
                text: text.to_string(),
                ..Default::default()
            };

            let output = ttrpc_client.text_chat(default_ctx(), &req).await;

            println!("output: {:?}", output);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
