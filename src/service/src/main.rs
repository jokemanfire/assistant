use std::{error::Error, process::exit};
use tokio::signal::unix::signal;
pub mod config;
pub mod dialogue_model;
pub mod server;
pub mod speech_to_text;


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let r = server::start_server().await;
    if r.is_err() {
        println!("Start server fail");
        exit(-1);
    }
    let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
    println!("server started");
    interrupt.recv().await;

    Ok(())
}
