use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Clone, Subcommand)]
pub enum ServerCommand {
    Start(StartCommand),
}

#[derive(Args, Clone)]
pub struct StartCommand {
    #[arg(short = 's', long = "server")]
    pub bootstrap_addr: Option<String>,

    #[arg(short = 'l', long = "listen", default_value = "127.0.0.1:50051")]
    pub listen_host: String,

    #[arg(short = 'i', long = "id")]
    pub id: Option<String>,

    #[arg(short = 'a', long = "layer", default_value = "cloud")]
    pub layer: String,

    #[arg(short = 'd', long = "data-path", default_value = ".laqista")]
    pub data_path: PathBuf,

    #[arg(long = "scheduler", default_value = "mean")]
    pub scheduler: String,

    #[arg(long = "initial-apps")]
    pub initial_apps: Vec<String>,
}
