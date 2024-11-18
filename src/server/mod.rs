pub mod cmd;
pub mod run;
pub mod server;

pub use cmd::*;
pub use run::*;
pub use server::*;

pub mod test_helpers {
    use std::sync::Once;

    use super::{ServerCommand, ServerRunner, StartCommand};

    pub static SERVER_THREAD: Once = Once::new();

    pub fn initialize() {
        SERVER_THREAD.call_once(|| {
            let command = ServerCommand::Start(StartCommand {
                bootstrap_addr: None,
                listen_host: "0.0.0.0:50051".to_owned(),
                id: None,
            });

            tokio::task::spawn(async { ServerRunner::new(command).run().await });
        });
    }
}
