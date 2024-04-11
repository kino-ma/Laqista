use clap::Parser;

use mless::{
    cmd::{Cli, Commands},
    server::ServerRunner,
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    use Commands::*;

    match cli.command {
        Server(subcmd) => {
            let mut runner = ServerRunner::new(subcmd);
            runner.run().await?;
        }
    };

    Ok(())
}
