pub mod cmd;

use std::error::Error;

use mac_address::MacAddressError;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tonic::{transport::Server as TransportServer, Request, Response, Status};
use uuid::Uuid;

use crate::proto::scheduler_client::SchedulerClient;
use crate::proto::scheduler_server::SchedulerServer;
use crate::proto::server_daemon_server::{ServerDaemon, ServerDaemonServer};
use crate::proto::{
    DestroyRequest, DestroyResponse, GetInfoRequest, GetInfoResponse, JoinRequest, MonitorRequest,
    MonitorResponse, PingResponse, ServerState, SpawnRequest, SpawnResponse,
};
use crate::scheduler::uninit::UninitScheduler;
use crate::scheduler::AuthoritativeScheduler;
use crate::utils::get_mac;
use crate::{GroupInfo, ServerInfo};

use self::cmd::{ServerCommand, StartCommand};

const DEFAULT_HOST: &'static str = "http://127.0.0.1:50051";

#[derive(Clone, Debug)]
pub struct ServerDaemonRuntime {
    info: ServerInfo,
    state: DaemonState,
}

#[derive(Clone, Debug)]
pub enum DaemonState {
    Starting,
    Running(GroupInfo),
    Uninitialized,
    Joining(String),
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
        let mut daemon = self.create_daemon(server_command, start_command)?;

        daemon.start().await
    }

    pub fn create_daemon(
        &self,
        _server_command: &ServerCommand,
        start_command: &StartCommand,
    ) -> Result<ServerDaemonRuntime, Box<dyn Error>> {
        let maybe_id = start_command.id.as_deref();
        let maybe_addr: Option<&str> = Some(&start_command.listen_host);
        let maybe_bootstrap_addr = start_command.bootstrap_addr.as_deref();

        ServerDaemonRuntime::with_optionals(maybe_id, maybe_addr, maybe_bootstrap_addr)
    }
}

impl ServerDaemonRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(id: &str) -> Result<Self, uuid::Error> {
        let id = Uuid::try_parse(id)?;
        let info = ServerInfo {
            id,
            addr: DEFAULT_HOST.to_owned(),
        };

        Ok(Self::with_info(&info))
    }

    pub fn with_info(info: &ServerInfo) -> Self {
        let info = info.clone();
        let state = DaemonState::Starting;

        Self { info, state }
    }

    pub fn with_optionals(
        maybe_id: Option<&str>,
        maybe_addr: Option<&str>,
        maybe_bootstrap_addr: Option<&str>,
    ) -> Result<Self, Box<dyn Error>> {
        let id = if let Some(id) = maybe_id {
            Uuid::try_parse(&id)?
        } else {
            ServerDaemonRuntime::gen_id()?
        };

        let addr = maybe_addr.unwrap_or(DEFAULT_HOST).to_owned();

        let info = ServerInfo { id, addr };

        let this = match maybe_bootstrap_addr {
            Some(bootstrap_addr) => Self::new_joining(&info, bootstrap_addr),
            None => Self::with_info(&info),
        };

        Ok(this)
    }

    pub fn new_joining(info: &ServerInfo, bootstrap_addr: &str) -> Self {
        let info = info.clone();
        let state = DaemonState::Joining(bootstrap_addr.to_owned());

        Self { info, state }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let next = self.start_service().await?;
            self.set_state(next);
        }
    }

    pub async fn start_service(&self) -> Result<DaemonState, Box<dyn Error>> {
        let addr = self.info.addr.parse()?;

        let state = &self.state;

        match state {
            #[allow(unused_must_use)]
            DaemonState::Uninitialized => {
                let (tx, mut rx) = mpsc::channel(1);
                let cancel_token = CancellationToken::new();

                let scheduler =
                    UninitScheduler::new(self.info.clone(), tx, cancel_token.child_token());

                let service = SchedulerServer::new(scheduler);
                let initializing_server = TransportServer::builder().add_service(service);

                // Ignoring this Future because we await for Cancellation later
                tokio::spawn(async move {
                    initializing_server.serve(addr).await;
                });

                cancel_token.cancelled().await;

                let new_state = rx.recv().await.ok_or("could not receive the scheduler")?;

                println!("Uninit scheduler successfully cancelled");

                Ok(new_state)
            }
            DaemonState::Joining(bootstrap_addr) => self.join_cluster(&bootstrap_addr).await,
            DaemonState::Running(group) => {
                let grpc_server =
                    TransportServer::builder().add_service(ServerDaemonServer::new(self.clone()));

                grpc_server.serve(addr).await?;

                Ok(DaemonState::Running(group.clone()))
            }
            DaemonState::Authoritative(scheduler) => {
                let grpc_server = TransportServer::builder()
                    .add_service(ServerDaemonServer::new(self.clone()))
                    .add_service(SchedulerServer::new(scheduler.clone()));

                grpc_server.serve(addr).await?;

                Ok(DaemonState::Authoritative(scheduler.clone()))
            }
            DaemonState::Starting => Ok(DaemonState::Uninitialized),
            DaemonState::Failed => {
                panic!("invalid state: {:?}", state)
            }
        }
    }

    fn set_state(&mut self, state: DaemonState) {
        self.state = state
    }

    pub async fn join_cluster(&self, addr: &str) -> Result<DaemonState, Box<dyn Error>> {
        println!("Joining a cluster over {}...", addr);

        let mut client = self.scheduler_client(addr).await?;

        let request = JoinRequest {
            server: Some(self.info.clone().into()),
        };
        let resp = client.join(request).await?;

        println!("JoinResponse: {:?}", resp);

        let group = resp
            .into_inner()
            .group
            .expect("Group in response cannot be empty")
            .try_into()?;

        Ok(DaemonState::Running(group))
    }

    pub fn create_cluster(&self) -> DaemonState {
        DaemonState::Uninitialized
    }

    async fn scheduler_client(
        &self,
        target_addr: &str,
    ) -> Result<SchedulerClient<Channel>, Box<dyn Error>> {
        Ok(SchedulerClient::connect(target_addr.to_owned()).await?)
    }

    fn gen_id() -> Result<Uuid, MacAddressError> {
        let mac = get_mac()?;
        Ok(Uuid::now_v6(&mac.bytes()))
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
        let state = &self.state;

        use DaemonState::*;
        let group = match &state {
            Starting | Uninitialized | Failed | Joining(_) => None,
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
        };

        let state: ServerState = state.clone().into();
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
        let id = Self::gen_id().expect("failed to generate id");
        let info = ServerInfo {
            id,
            addr: "127.0.0.1:50051".to_owned(),
        };

        let state = DaemonState::Starting;

        Self { info, state }
    }
}
