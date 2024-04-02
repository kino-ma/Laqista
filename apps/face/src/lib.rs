pub mod detector;
pub mod server;

pub mod proto {
    tonic::include_proto!("face");
}

pub use detector::*;
