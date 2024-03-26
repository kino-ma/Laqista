use clap::{Arg, ArgAction, ArgMatches, Command};
use mless::server::Server;

fn main() {
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

    let matches = command.get_matches();

    match matches.subcommand() {
        Some(("server", server_matches)) => run_server(server_matches),
        Some((subcommand, args)) => {
            panic!("unknown command: {:?}, with args = {:?}", subcommand, args)
        }
        None => unreachable!(),
    }
}

fn run_server(matches: &ArgMatches) {
    let server = Server::new();

    if matches.get_flag("start") {
        server.start().expect("failed to start the server");
    } else {
        println!("no flags!");
    }
}
