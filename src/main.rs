use clap::{Arg, ArgAction, Command};

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
        Some(("server", server_matches)) => {
            if server_matches.get_flag("start") {
                println!("starting the server!");
            } else {
                println!("no flags!");
            }
        }
        Some((subcommand, args)) => {
            panic!("unknown command: {:?}, with args = {:?}", subcommand, args)
        }
        None => unreachable!(),
    }
}
