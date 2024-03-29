pub mod cmd;

use std::error::Error;

use std::net::SocketAddr;
use std::pin::pin;

use futures::future;
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
use crate::proxy::create_reverse_proxy;
use crate::scheduler::mean::MeanGpuScheduler;
use crate::scheduler::uninit::UninitScheduler;
use crate::scheduler::{AuthoritativeScheduler, Cluster};
use crate::{GroupInfo, ServerInfo};

use self::cmd::{ServerCommand, StartCommand};

const DEFAULT_HOST: &'static str = "127.0.0.1:50051";

#[derive(Clone, Debug)]
pub struct ServerDaemonRuntime {
    info: ServerInfo,
    socket: SocketAddr,
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
        let socket = info.as_socket().expect("failed to parse host in info");
        let state = DaemonState::Starting;

        Self {
            info,
            socket,
            state,
        }
    }

    pub fn with_optionals(
        maybe_id: Option<&str>,
        maybe_addr: Option<&str>,
        maybe_bootstrap_addr: Option<&str>,
    ) -> Result<Self, Box<dyn Error>> {
        let host = maybe_addr.unwrap_or(DEFAULT_HOST).to_owned();

        let info = if let Some(id) = maybe_id {
            let id = Uuid::try_parse(&id)?;
            ServerInfo::with_id(&host, &id)
        } else {
            ServerInfo::new(&host)
        };

        let this = match maybe_bootstrap_addr {
            Some(bootstrap_addr) => Self::new_joining(&info, bootstrap_addr),
            None => Self::with_info(&info),
        };

        Ok(this)
    }

    pub fn new_joining(info: &ServerInfo, bootstrap_addr: &str) -> Self {
        let info = info.clone();
        let socket = info.as_socket().expect("failed to parse host in info");
        let state = DaemonState::Joining(bootstrap_addr.to_owned());

        Self {
            info,
            socket,
            state,
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let next = self.start_service().await?;
            self.set_state(next);
        }
    }

    pub async fn start_service(&self) -> Result<DaemonState, Box<dyn Error>> {
        let state = &self.state;

        match state {
            #[allow(unused_must_use)]
            DaemonState::Uninitialized => {
                println!("Uninitialized. Waiting for others to join...");

                let (tx, mut rx) = mpsc::channel(1);
                let cancel_token = CancellationToken::new();

                let scheduler =
                    UninitScheduler::new(self.info.clone(), tx, cancel_token.child_token());

                let service = SchedulerServer::new(scheduler);
                let initializing_server = TransportServer::builder().add_service(service);

                let serving_future = initializing_server.serve(self.socket);

                // Terminate serving_future by selecting another future
                let new_state = match future::select(pin!(serving_future), pin!(rx.recv())).await {
                    future::Either::Left(_) => unreachable!(),
                    future::Either::Right((state, _)) => state,
                }
                .ok_or("could not receive the scheduler")?;

                println!("Uninit scheduler successfully cancelled");

                Ok(new_state)
            }
            DaemonState::Joining(bootstrap_addr) => {
                println!("Joining to a cluster...");
                self.join_cluster(&bootstrap_addr).await
            }
            DaemonState::Running(group) => {
                println!("Running a new server...");

                let handler = create_reverse_proxy("mypackage", "myaddr").await?;
                let router = TransportServer::builder()
                    .add_service(ServerDaemonServer::new(self.clone()))
                    .into_router()
                    .route("/", handler);

                axum::Server::bind(&self.socket)
                    .serve(router.into_make_service())
                    .await?;

                Ok(DaemonState::Running(group.clone()))
            }
            DaemonState::Authoritative(scheduler) => {
                println!("Running an Authoritative server...");

                let grpc_server = TransportServer::builder()
                    .add_service(ServerDaemonServer::new(self.clone()))
                    .add_service(SchedulerServer::new(scheduler.clone()));

                grpc_server.serve(self.socket).await?;

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
        let resp = client.join(request).await?.into_inner();

        println!("JoinResponse: {:?}", resp);

        let group = resp
            .group
            .expect("Group in response cannot be empty")
            .try_into()?;

        if resp.is_scheduler {
            let other = resp
                .our_group
                .expect("Other group cannot be empty when becoming a scheduler");

            let cluster = Cluster::with_group(&group);
            let other_cluster = Cluster::with_group(&other.try_into()?);

            let scheduler =
                AuthoritativeScheduler::new(cluster, other_cluster, Box::new(MeanGpuScheduler {}));
            Ok(DaemonState::Authoritative(scheduler))
        } else {
            Ok(DaemonState::Running(group))
        }
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
        let info = ServerInfo::new(DEFAULT_HOST);
        let socket = DEFAULT_HOST.parse().expect("failed to parse DEFAULT_HOST");
        let state = DaemonState::Starting;

        Self {
            info,
            socket,
            state,
        }
    }
}
