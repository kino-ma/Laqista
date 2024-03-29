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

    let (tx, mut rx) = mpsc::channel(10);
    tokio::spawn(async move {
        println!("start thread");
        let monit = PowerMonitor::new();
        monit.start(tx).await;
    });

    tokio::spawn(async move {
        println!("start listen thread");
        while let Some(metrics) = rx.recv().await {
            println!(
                "received metrics: utilization = {:.3}%",
                metrics.gpu.utilization_ratio() * 100.
            );
        }
        println!("end listen thread");
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
