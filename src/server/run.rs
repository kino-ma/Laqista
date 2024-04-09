use std::net::SocketAddr;
use std::pin::pin;

use futures::future;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tonic::transport::{server::Router, Channel, Server as TransportServer};

use crate::proto::{scheduler_client::SchedulerClient, JoinRequest};
use crate::report::MetricsReporter;
use crate::scheduler::uninit::UninitScheduler;
use crate::scheduler::AuthoritativeScheduler;
use crate::{
    proto::{scheduler_server::SchedulerServer, server_daemon_server::ServerDaemonServer},
    scheduler::{mean::MeanGpuScheduler, Cluster},
};
use crate::{Error, GroupInfo, Result, ServerInfo};

use crate::server::{ServerCommand, StartCommand};

use super::ServerDaemon;

use face::{proto as face_proto, server::DetectServer};

pub const DEFAULT_HOST: &'static str = "127.0.0.1:50051";

pub struct ServerRunner {
    command: ServerCommand,
    socket: SocketAddr,
    rx: Mutex<mpsc::Receiver<DaemonState>>,
    tx: mpsc::Sender<DaemonState>,
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

impl ServerRunner {
    pub fn new(command: ServerCommand) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let rx = Mutex::new(rx);
        let socket = DEFAULT_HOST.parse().expect("failed to parse default host");

        Self {
            command,
            socket,
            rx,
            tx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        use ServerCommand::*;
        let command = self.command.clone();

        match &command {
            Start(subcmd) => self.run_start(&subcmd),
        }
        .await
    }

    pub async fn run_start(&mut self, start_command: &StartCommand) -> Result<()> {
        let info = self.create_info(start_command)?;
        let mut state = self.determine_state(start_command);

        loop {
            let daemon = self.create_daemon(info.clone(), state.clone());
            let service_future = self.start_service(daemon, state);

            let mut rx = self.rx.lock().await;
            let rcvd_future = rx.recv();

            // Terminate serving_future by selecting another future
            state = match future::select(pin!(service_future), pin!(rcvd_future)).await {
                future::Either::Left((state, _)) => state?,
                future::Either::Right((state, _)) => {
                    state.ok_or(Error::Text("received None state from sender".to_owned()))?
                }
            };
        }
    }

    pub async fn start_service(
        &self,
        daemon: ServerDaemon,
        state: DaemonState,
    ) -> Result<DaemonState> {
        let server = daemon.runtime.info.clone();

        match state {
            #[allow(unused_must_use)]
            DaemonState::Uninitialized => {
                println!("Uninitialized. Waiting for others to join...");
                self.start_uninitialized(server).await
            }
            DaemonState::Joining(bootstrap_addr) => {
                println!("Joining to a cluster...");
                let server = ServerInfo::new(&self.socket.to_string());
                self.join_cluster(server, &bootstrap_addr).await
            }
            DaemonState::Running(group) => {
                println!("Running a new server...");
                println!("group = {:?}", group);

                self.start_running(daemon, server, group).await
            }
            DaemonState::Authoritative(scheduler) => {
                println!("Running an Authoritative server...");
                self.start_authoritative(daemon, scheduler).await
            }
            DaemonState::Starting => Ok(DaemonState::Uninitialized),
            DaemonState::Failed => {
                panic!("invalid state: {:?}", state)
            }
        }
    }

    async fn start_uninitialized(&self, server: ServerInfo) -> Result<DaemonState> {
        let cancel_token = CancellationToken::new();

        let scheduler = UninitScheduler::new(server, self.tx.clone(), cancel_token.child_token());

        let service = SchedulerServer::new(scheduler);
        let initializing_server = TransportServer::builder().add_service(service);

        initializing_server.serve(self.socket).await?;

        println!("Uninit scheduler successfully cancelled");

        unreachable!("state should be sent from UninitScheduler first");
    }

    async fn join_cluster(&self, server: ServerInfo, addr: &str) -> Result<DaemonState> {
        println!("Joining a cluster over {}...", addr);

        let mut client = self.scheduler_client(addr).await?;

        let request = JoinRequest {
            server: Some(server.clone().into()),
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

    async fn start_authoritative(
        &self,
        daemon: ServerDaemon,
        scheduler: AuthoritativeScheduler,
    ) -> Result<DaemonState> {
        let server = daemon.runtime.info.clone();

        let scheduler_info = scheduler
            .runtime
            .lock()
            .unwrap()
            .cluster
            .group
            .scheduler_info
            .clone();
        let reporter_token = self.start_reporter(server.clone(), scheduler_info);

        let grpc_server = self
            .common_services(daemon)
            .add_service(SchedulerServer::new(scheduler.clone()));

        grpc_server.serve(self.socket).await?;

        println!("cancel reporter (authoritative)");
        reporter_token.cancel();

        Ok(DaemonState::Authoritative(scheduler.clone()))
    }

    async fn start_running(
        &self,
        daemon: ServerDaemon,
        server: ServerInfo,
        group: GroupInfo,
    ) -> Result<DaemonState> {
        let reporter_token = self.start_reporter(server.clone(), group.scheduler_info.clone());

        let grpc_router = self.common_services(daemon);

        grpc_router.serve(self.socket).await?;

        println!("cancel reporter (running)");
        reporter_token.cancel();

        Ok(DaemonState::Running(group.clone()))
    }

    fn start_reporter(&self, server: ServerInfo, scheduler: ServerInfo) -> CancellationToken {
        let token = CancellationToken::new();
        let cloned = token.clone();

        let mut reporter = MetricsReporter::new(server, scheduler);
        tokio::spawn(async move { reporter.start(cloned).await });

        token
    }

    fn common_services(&self, daemon: ServerDaemon) -> Router {
        TransportServer::builder()
            .add_service(ServerDaemonServer::new(daemon))
            .add_service(hello::proto::greeter_server::GreeterServer::new(
                hello::MyGreeter::default(),
            ))
            .add_service(face_proto::detector_server::DetectorServer::new(
                DetectServer {},
            ))
    }

    fn create_daemon(&self, info: ServerInfo, state: DaemonState) -> ServerDaemon {
        ServerDaemon::with_state(state, info, self.tx.clone())
    }

    fn create_info(&self, start_command: &StartCommand) -> Result<ServerInfo> {
        let host = &start_command.listen_host;

        match &start_command.id {
            Some(id) => ServerInfo::with_id_str(&id, host),
            None => Ok(ServerInfo::new(host)),
        }
    }

    pub fn determine_state(&self, start_command: &StartCommand) -> DaemonState {
        let maybe_bootstrap_addr = start_command.bootstrap_addr.as_deref();

        match maybe_bootstrap_addr {
            Some(bootstrap_addr) => DaemonState::Joining(bootstrap_addr.to_owned()),
            None => DaemonState::Starting,
        }
    }

    async fn scheduler_client(&self, target_addr: &str) -> Result<SchedulerClient<Channel>> {
        Ok(SchedulerClient::connect(target_addr.to_owned()).await?)
    }
}
