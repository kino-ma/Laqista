pub mod detector;
mod server;

mod proto {
    tonic::include_proto!("face");
}

pub use detector::*;
