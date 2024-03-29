use std::error::Error;

use clap::Parser;

use mless::{
    cmd::{Cli, Commands},
    monitor::PowerMonitor,
    server::ServerRunner,
};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    tokio::spawn(async {
        println!("start thread");
        let (tx, _rx) = mpsc::channel(1);
        let monit = PowerMonitor::new();
        monit.start(tx).await;
        println!("end thread");
    });

    use Commands::*;

    match cli.command {
        Server(subcmd) => {
            let runner = ServerRunner::new(subcmd);
            runner.run().await?;
        }
    };

    Ok(())
}
