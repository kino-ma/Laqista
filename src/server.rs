pub mod cmd;

use std::borrow::BorrowMut;
use std::error::Error;
use std::sync::{Arc, Mutex};

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

#[derive(Clone, Debug)]
pub struct ServerDaemonRuntime {
    info: ServerInfo,
    state: Arc<Mutex<DaemonState>>,
    bootstrap_addr: Option<String>,
}

#[derive(Clone, Debug)]
pub enum DaemonState {
    Starting,
    Running(GroupInfo),
    Uninitialized,
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
                start_command.bootstrap_addr.as_ref().map(|x| x.as_str()),
            ))
        } else {
            // Non initialized. Craeting new server
            Ok(ServerDaemonRuntime::default())
        }
    }
}

impl ServerDaemonRuntime {
    pub fn new(id: Uuid, addr: &str, bootstrap_addr: Option<&str>) -> Self {
        let info = ServerInfo {
            id,
            addr: addr.to_owned(),
        };
        let state = Arc::new(Mutex::new(DaemonState::Starting));
        let bootstrap_addr = bootstrap_addr.map(|s| s.to_owned());

        Self {
            info,
            state,
            bootstrap_addr,
        }
    }

    pub fn new_with_id(id: Uuid, addr: &str, bootstrap_addr: Option<&str>) -> Self {
        Self::new(id, addr, bootstrap_addr)
    }

    pub fn create() -> Result<Self, Box<dyn Error>> {
        let mac = get_mac()?;
        let id = Uuid::now_v6(&mac.bytes());

        Ok(Self::new(id, "127.0.0.1:50051", None))
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        loop {
            self.start_service().await?;

            let lock = self.state.lock().unwrap();
            match *lock {
                DaemonState::Starting | DaemonState::Uninitialized | DaemonState::Failed => {
                    panic!("invalid state in loop")
                }
                _ => (),
            }
        }
    }

    pub async fn start_service(&self) -> Result<DaemonState, Box<dyn Error>> {
        let addr = self.info.addr.parse()?;

        let state = self.state.lock().unwrap().clone();

        match state {
            #[allow(unused_must_use)]
            DaemonState::Uninitialized => {
                let (tx, mut rx) = mpsc::channel(1);
                let cancel_token = CancellationToken::new();

                let scheduler =
                    UninitScheduler::new(self.info.clone(), tx, cancel_token.child_token());

                let service = SchedulerServer::new(scheduler.clone());
                let initializing_server = TransportServer::builder().add_service(service);

                // Ignoring this Future because we await for Cancellation later
                initializing_server.serve(addr);

                let new_state = rx.recv().await.ok_or("could not receive the scheduler")?;

                cancel_token.cancelled().await;
                println!("Uninit scheduler successfully cancelled");

                Ok(new_state)
            }
            DaemonState::Running(group) => {
                let grpc_server =
                    TransportServer::builder().add_service(ServerDaemonServer::new(self.clone()));

                grpc_server.serve(addr).await?;

                Ok(DaemonState::Running(group))
            }
            DaemonState::Authoritative(scheduler) => {
                let grpc_server = TransportServer::builder()
                    .add_service(ServerDaemonServer::new(self.clone()))
                    .add_service(SchedulerServer::new(scheduler.clone()));

                grpc_server.serve(addr).await?;

                Ok(DaemonState::Authoritative(scheduler))
            }
            DaemonState::Starting | DaemonState::Failed => {
                panic!("invalid state: {:?}", state)
            }
        }
    }

    pub async fn bootstrap(&mut self) -> Result<(), Box<dyn Error>> {
        let state = if let Some(addr) = &self.bootstrap_addr {
            self.join_cluster(&addr).await?
        } else {
            self.create_cluster()
        };

        self.set_state(state);

        Ok(())
    }

    fn set_state(&mut self, state: DaemonState) {
        let mut lock = self.state.lock().unwrap();
        let ptr = lock.borrow_mut();
        **ptr = state;
    }

    pub async fn join_cluster(&self, addr: &str) -> Result<DaemonState, Box<dyn Error>> {
        let mut client = self.scheduler_client(addr).await?;

        let request = JoinRequest {
            server: Some(self.info.clone().into()),
        };
        let resp = client.join(request).await?;

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
}

#[tonic::async_trait]
impl ServerDaemon for ServerDaemonRuntime {
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> Result<Response<GetInfoResponse>, Status> {
        println!("GetInfo called!");

        let server = Some(self.info.clone().into());
        let state = self.state.lock().unwrap().clone();

        use DaemonState::*;
        let group = match &state {
            Starting => None,
            Running(group) => Some(group.clone().into()),
            Uninitialized => None,
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

        let state: ServerState = state.into();
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
