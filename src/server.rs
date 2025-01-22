use prost::Message;
use ttrpc::context::Context;
use ttrpc::server::{self, Server};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

mod proto {
    include!("protocols/api_ttrpc.rs");
}

struct HelloServiceImpl {
   
}

#[ttrpc::async_trait]
impl proto::hello_service::HelloService for HelloServiceImpl {
    async fn hello_world(
        &self,
        _ctx: &Context,
        request: proto::HelloRequest,
    ) -> Result<proto::HelloResponse, ttrpc::Status> {
        let response = proto::HelloResponse {
            message: format!("Hello, {}!", request.name),
        };
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:50051".parse()?;
    let service = Arc::new(Mutex::new(HelloServiceImpl {}));
    let mut server = Server::new()?;
    server.register_service(proto::hello_service_server::HelloServiceServer::new(service))?;
    println!("Starting ttrpc server on {}", addr);
    server.start(addr).await?;
    Ok(())
}