use std::error::Error;

use clap::{Arg, ArgAction, ArgMatches, Command};
use mless::server::Daemon;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let command = get_command();
    let matches = command.get_matches();

    match matches.subcommand() {
        Some(("server", server_matches)) => run_server(server_matches).await,
        Some((subcommand, args)) => {
            panic!("unknown command: {:?}, with args = {:?}", subcommand, args)
        }
        None => unreachable!(),
    }

    Ok(())
}

fn get_command() -> Command {
    let command_server = Command::new("server").arg(
        Arg::new("start")
            .short('s')
            .long("start")
            .help("start a nwe MLess Server")
            .action(ArgAction::SetTrue),
    );

    let command = Command::new("mless")
        .about("MLess CLI")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(command_server);

    return command;
}

async fn run_server(matches: &ArgMatches) {
    let server = Daemon::default();

    if matches.get_flag("start") {
        server.start().await.expect("failed to start the server");
    } else {
        println!("no flags!");
    }
}
