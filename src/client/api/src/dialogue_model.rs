use protos::{model, model_ttrpc};
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
    let client = Client::connect(DIALOGUE_MODEL_API_URL).unwrap();
    // tokio::time::sleep(Duration::from_secs(1)).await;

    let ttrpc_client = model_ttrpc::ModelServiceClient::new(client);
    let req = model::TextRequest {
        text: input_text.to_string(),
        ..Default::default()
    };
    // tokio::time::sleep(Duration::from_secs(3)).await;
    let output = ttrpc_client.text_chat(default_ctx(), &req).await?;
    // let test = "test";
    // println!("output: {}", output.text);
    Ok(output.text)
    // Ok(test.to_string())
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
