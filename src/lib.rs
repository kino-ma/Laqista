pub mod server;

mod utils;

pub mod proto {
    tonic::include_proto!("mless");
}
