use local::runmodel::{IoOption, ModelRunner};
use std::{error::Error, process::exit};
use tokio::signal::unix::signal;
use log::{info, error};
pub mod config;
pub mod dialogue_model;
pub mod local;
pub mod server;
pub mod speech_to_text;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let r = server::start_server().await;
    if r.is_err() {
        error!("Start server fail");
        exit(-1);
    }
    let mut interrupt = signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
    info!("server started");
    interrupt.recv().await;

    Ok(())
}
