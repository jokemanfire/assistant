use reqwest::Client;
use serde_json::json;
use rodio::Sink;

pub async fn play_response(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let response = client
        .post("https://api.text-to-speech.com/synthesize")
        .json(&json!({
            "text": text
        }))
        .send()
        .await?;

    let audio_data = response.bytes().await?;

    // 使用 rodio 播放音频
    let (_stream, stream_handle) = rodio::OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(rodio::Decoder::new(std::io::Cursor::new(audio_data))?);
    sink.sleep_until_end();

    Ok(())
}