use std::error::Error;
use tokio::{runtime::Runtime, signal::unix::signal};
pub mod config;
pub mod dialogue_model;
pub mod server;
pub mod speech_to_text;
use log::LevelFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    simple_logging::log_to_stderr(LevelFilter::Trace);
    let r = server::start_server().await;
    let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
    println!("server started {:?}", r);
    interrupt.recv().await;

    Ok(())
}
