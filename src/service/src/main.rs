use local::runmodel::{IoOption, ModelRunner};
use log::{error, info};
use std::{error::Error, process::exit};
use tokio::signal::unix::signal;
pub mod config;
pub mod local;
pub mod server;
pub mod modeldeal;


// pub struct Service {
//     config: Config,
//     model_manager: ModelManager,
// }

// impl Service {
//     pub async fn new() -> anyhow::Result<Self> {
//         let config = Config::builder()
//             .add_source(config::File::with_name("default.toml"))
//             .build()?;
            
//         let mut manager = ModelManager::new(config.clone());
//         manager.init().await.unwrap();

//         Ok(Self {
//             config,
//             model_manager: manager,
//         })
//     }

//     pub async fn chat(&self, message: String) -> anyhow::Result<String> {
//         // 如果启用了本地模型,使用本地模型
//         if self.config.get_bool("local_model.enabled")? {
//             self.model_manager.run_local_model(message).await
//         } else {
//             // 使用远程模型的逻辑
//             todo!()
//         }
//     }
// } 

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
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
