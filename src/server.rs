pub mod cmd;

use std::error::Error;

use tonic::{transport::Server as TransportServer, Request, Response, Status};
use uuid::Uuid;

use crate::proto::server_daemon_server::{ServerDaemon, ServerDaemonServer};
use crate::proto::{
    DestroyRequest, DestroyResponse, GetInfoRequest, GetInfoResponse, MonitorRequest,
    MonitorResponse, PingResponse, ServerState, SpawnRequest, SpawnResponse,
};
use crate::scheduler::AuthoritativeScheduler;
use crate::utils::get_mac;
use crate::{GroupInfo, ServerInfo};

use self::cmd::{ServerCommand, StartCommand};

#[derive(Clone, Debug)]
pub struct ServerDaemonRuntime {
    info: ServerInfo,
    state: DaemonState,
}

#[derive(Clone, Debug)]
pub enum DaemonState {
    Uninitialized,
    Running(GroupInfo),
    Authoritative(AuthoritativeScheduler),
    Failed,
}

pub struct ServerRunner {
    command: ServerCommand,
}

impl ServerRunner {
    pub fn new(command: ServerCommand) -> Self {
        Self { command }
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
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
    ) -> Result<(), Box<dyn Error>> {
        let daemon = self.create_daemon(server_command, start_command)?;

        daemon.start().await
    }

    pub fn create_daemon(
        &self,
        _server_command: &ServerCommand,
        start_command: &StartCommand,
    ) -> Result<ServerDaemonRuntime, Box<dyn Error>> {
        if let Some(id) = &start_command.id {
            let id = Uuid::try_parse(&id)?;
            Ok(ServerDaemonRuntime::new_with_id(
                id,
                &start_command.listen_host,
            ))
        } else {
            // Non initialized. Craeting new server
            Ok(ServerDaemonRuntime::default())
        }
    }
}

impl ServerDaemonRuntime {
    pub fn new(id: Uuid, addr: &str) -> Self {
        let info = ServerInfo {
            id,
            addr: addr.to_owned(),
        };
        let state = DaemonState::Uninitialized;

        Self { info, state }
    }

    pub fn new_with_id(id: Uuid, addr: &str) -> Self {
        Self::new(id, addr)
    }

    pub fn create() -> Result<Self, Box<dyn Error>> {
        let mac = get_mac()?;
        let id = Uuid::now_v6(&mac.bytes());

        Ok(Self::new(id, "127.0.0.1:50051"))
    }

    pub async fn start(self) -> Result<(), Box<dyn Error>> {
        let addr = self.info.addr.parse()?;

        println!("starting server... ({})", self.info.id);

        TransportServer::builder()
            .add_service(ServerDaemonServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl ServerDaemon for ServerDaemonRuntime {
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> Result<Response<GetInfoResponse>, Status> {
        println!("GetInfo called!");

        let server = Some(self.info.clone().into());

        use DaemonState::*;
        let group = match &self.state {
            Uninitialized => None,
            Running(group) => Some(group.clone().into()),
            Authoritative(scheduler) => Some(
                scheduler
                    .runtime
                    .lock()
                    .unwrap()
                    .cluster
                    .group
                    .clone()
                    .into(),
            ),
            Failed => None,
        };

        let state: ServerState = self.state.clone().into();
        let state = state.into();

        let resposne = GetInfoResponse {
            server,
            group,
            state,
        };

        Ok(Response::new(resposne))
    }

    async fn ping(&self, _request: Request<()>) -> Result<Response<PingResponse>, Status> {
        println!("got ping!");

        let resposne = PingResponse { success: true };

        Ok(Response::new(resposne))
    }

    async fn monitor(
        &self,
        _request: Request<MonitorRequest>,
    ) -> Result<Response<MonitorResponse>, Status> {
        Ok(Response::new(MonitorResponse { windows: vec![] }))
    }
    async fn spawn(
        &self,
        _request: Request<SpawnRequest>,
    ) -> Result<Response<SpawnResponse>, Status> {
        Ok(Response::new(SpawnResponse {
            success: true,
            deployment: None,
            server: None,
        }))
    }
    async fn destroy(
        &self,
        _request: Request<DestroyRequest>,
    ) -> Result<Response<DestroyResponse>, Status> {
        Ok(Response::new(DestroyResponse { success: true }))
    }
}

impl Default for ServerDaemonRuntime {
    fn default() -> Self {
        Self::create().expect("failed to create default daemon")
    }
}
