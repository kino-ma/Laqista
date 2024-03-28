use clap::{Parser, Subcommand};

use crate::server::cmd::ServerCommand;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(subcommand)]
    Server(ServerCommand),
}
