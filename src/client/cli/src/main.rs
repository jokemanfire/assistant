use std::{error::Error, io};


fn read_input() -> String {
    loop {
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .expect("Failed to read line");
        if !answer.is_empty() && answer != "\n" && answer != "\r\n" {
            return answer.trim().to_string();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // simple_logging::log_to_stderr(LevelFilter::Trace);
    loop {
        println!("USER:");
        let input = read_input();
        let r = api::dialogue_model::dialogue_model(input).await;
        println!("AI: {:?}", r.unwrap_or_default());
        // tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}
