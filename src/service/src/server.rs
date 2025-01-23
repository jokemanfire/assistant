use prost::Message;
use ttrpc::context::Context;
use ttrpc::r#async::Server;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use protos::model_ttrpc;




async fn start_server() -> Result<(), Box<dyn Error>> {
    // let addr = "127.0.0.1:50051".parse()?;
    // let service = Arc::new(Mutex::new(HelloServiceImpl {}));
    // let mut server = Server::new()?;
    // server.register_service(proto::hello_service_server::HelloServiceServer::new(service))?;
    // println!("Starting ttrpc server on {}", addr);
    // server.start(addr).await?;
    Ok(())
}