pub mod server;

use thiserror::Error;

// #[derive(Error, Debug)]
// pub enum ServerError {
//     #[error("Failed to parse address: {0}")]
//     AddressParseError(#[from] std::net::AddrParseError),
//     #[error("Failed to create server: {0}")]
//     ServerCreationError(#[from] ttrpc::server::Error),
//     #[error("Failed to register service: {0}")]
//     ServiceRegistrationError(#[from] ttrpc::server::Error),
//     #[error("Failed to start server: {0}")]
//     ServerStartError(#[from] ttrpc::server::Error),
// }
