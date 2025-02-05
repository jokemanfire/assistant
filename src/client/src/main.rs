use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::error::Error;
use std::sync::mpsc::channel;
use std::thread;
use reqwest::Client;
use serde_json::json;
use rodio::source::Source;
use rodio::{Decoder, OutputStream, Sink};

const SPEECH_TO_TEXT_API_URL: &str = "unix:///tmp/model-service.sock";
const DIALOGUE_MODEL_API_URL: &str = "unix:///tmp/model-service.sock";
const TEXT_TO_SPEECH_API_URL: &str = "unix:///tmp/model-service.sock";
const API_KEY: &str = "your_api_key";

async fn speech_to_text(audio_data: Vec<u8>) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let res = client.post(SPEECH_TO_TEXT_API_URL)
       .header("Authorization", format!("Bearer {}", API_KEY))
       .body(audio_data)
       .send()
       .await?;
    let text = res.text().await?;
    Ok(text)
}

async fn dialogue_model(input_text: String) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let res = client.post(DIALOGUE_MODEL_API_URL)
       .header("Authorization", format!("Bearer {}", API_KEY))
       .json(&json!({ "input": input_text }))
       .send()
       .await?;
    let output = res.text().await?;
    Ok(output)
}

async fn text_to_speech(text: String) -> Result<Vec<u8>, Box<dyn Error>> {
    let client = Client::new();
    let res = client.post(TEXT_TO_SPEECH_API_URL)
       .header("Authorization", format!("Bearer {}", API_KEY))
       .json(&json!({ "text": text }))
       .send()
       .await?;
    let audio_data = res.bytes().await?.to_vec();
    Ok(audio_data)
}

fn play_audio(audio_data: Vec<u8>) -> Result<(), Box<dyn Error>> {
    let buf_reader = BufReader::new(audio_data);
    let source = Decoder::new(buf_reader)?;
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Conect to ttrpc server
    let addr = "127.0.0.1:50051";

    let clinet = HelloServiceClient::new(Client::connect(addr).await?);
    let request = HelloRequest {
        name: "world".to_string(),
    };
    let response = client.hello_world(Context::default(), &request).await?;
    
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No input device found")?;
    let config = device.default_input_config()?;

    let (tx, rx) = channel();
    let err_fn = move |err| eprintln!("an error occurred on stream: {}", err);

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            tx.send(data.to_vec()).unwrap();
        },
        err_fn,
        None,
    )?;
    stream.play()?;

    let mut audio_data = Vec::new();
    loop {
        if let Ok(data) = rx.try_recv() {
            audio_data.extend_from_slice(&data);
            if audio_data.len() > 1024 * 1024 { 
                let text = speech_to_text(audio_data.clone()).await?;
                let response = dialogue_model(text).await?;
                let audio_output = text_to_speech(response).await?;
                play_audio(audio_output)?;
                audio_data.clear();
            }
        }
        thread::sleep(std::time::Duration::from_millis(100));
    }
}