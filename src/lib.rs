use uuid::Uuid;

pub mod server;

mod utils;

pub mod proto {
    tonic::include_proto!("mless");
}

#[derive(Clone, Debug)]
pub struct ServerInfo {
    id: Uuid,
    addr: String,
}
