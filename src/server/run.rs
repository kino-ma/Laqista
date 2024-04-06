use crate::Result;

use crate::server::{ServerCommand, ServerDaemonRuntime, StartCommand};

pub struct ServerRunner {
    command: ServerCommand,
}

impl ServerRunner {
    pub fn new(command: ServerCommand) -> Self {
        Self { command }
    }

    pub async fn run(&self) -> Result<()> {
        use ServerCommand::*;

        match &self.command {
            Start(subcmd) => self.run_start(&self.command, &subcmd),
        }
        .await
    }

    pub async fn run_start(
        &self,
        server_command: &ServerCommand,
        start_command: &StartCommand,
    ) -> Result<()> {
        let mut daemon = self.create_daemon(server_command, start_command)?;

        daemon.start().await
    }

    pub fn create_daemon(
        &self,
        _server_command: &ServerCommand,
        start_command: &StartCommand,
    ) -> Result<ServerDaemonRuntime> {
        let maybe_id = start_command.id.as_deref();
        let maybe_addr: Option<&str> = Some(&start_command.listen_host);
        let maybe_bootstrap_addr = start_command.bootstrap_addr.as_deref();

        ServerDaemonRuntime::with_optionals(maybe_id, maybe_addr, maybe_bootstrap_addr)
    }
}
