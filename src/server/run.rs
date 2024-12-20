use std::net::SocketAddr;
use std::pin::pin;
use std::str::FromStr;
use std::time::Duration;

use face::server::FaceServer;
use futures::future;
use local_ip_address::local_ip;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tonic::transport::{server::Router, Channel, Server as TransportServer};
use tonic_middleware::MiddlewareFor;

use crate::deployment::database::{DeploymentDatabase, Target};
use crate::proto::{scheduler_client::SchedulerClient, JoinRequest};
use crate::report::MetricsReporter;
use crate::scheduler::dew::DewScheduler;
use crate::scheduler::fog::FogScheduler;
use crate::scheduler::interface::DeploymentScheduler;
use crate::scheduler::round::RoundRobbinScheduler;
use crate::scheduler::{AuthoritativeScheduler, Cluster};
use crate::{
    proto::{scheduler_server::SchedulerServer, server_daemon_server::ServerDaemonServer},
    scheduler::mean::MeanScheduler,
};
use crate::{Error, GroupInfo, Result, ServerInfo};

use crate::server::{ServerCommand, StartCommand};

use super::middleware::MetricsMiddleware;
use super::ServerDaemon;

use hello::proto::greeter_server::GreeterServer;

#[cfg(feature = "face")]
use face::proto as face_proto;

pub const DEFAULT_HOST: &'static str = "127.0.0.1:50051";
pub const DEFAULT_STATS_CHANNELS: usize = 0x1000;

pub struct ServerRunner {
    command: ServerCommand,
    // FIXME: socket is not known at instantiation of this struct.
    // Instead, it will be known when it is the "start" command.
    socket: SocketAddr,
    database: DeploymentDatabase,
    rx: Mutex<StateReceiver>,
    tx: StateSender,
}

pub type StateSender = mpsc::Sender<StateCommand>;
pub type StateReceiver = mpsc::Receiver<StateCommand>;

#[derive(Clone, Debug)]
pub struct AppMetric {
    pub app: String,
    pub service: String,
    pub rpc: String,
    pub elapsed: Duration,
}
pub type AppMetricSender = mpsc::Sender<AppMetric>;
pub type AppMetricReceiver = mpsc::Receiver<AppMetric>;

#[derive(Clone, Debug)]
pub enum StateCommand {
    Update(DaemonState),
    BecomeScheduler(Cluster),
    Keep,
}

#[derive(Clone, Debug)]
pub enum DaemonState {
    Cloud(GroupInfo),
    Fog(FogScheduler),
    Dew(DewScheduler),
    Joining(String),
    Authoritative(AuthoritativeScheduler),
    Failed,
}

impl ServerRunner {
    pub fn new(command: ServerCommand) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let rx = Mutex::new(rx);
        let socket = DEFAULT_HOST.parse().expect("failed to parse default host");
        let database = DeploymentDatabase::default(tx.clone());

        Self {
            command,
            socket,
            database,
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
        self.socket = Self::get_socket(start_command)?;
        self.database =
            DeploymentDatabase::read_dir(start_command.data_path.clone(), self.tx.clone()).unwrap();

        let info = self.create_info(start_command)?;

        let mut state = self.determine_state(start_command, &info);

        loop {
            let daemon = self.create_daemon(info.clone(), state.clone());
            let service_future = self.start_service(daemon, state.clone());

            let mut rx = self.rx.lock().await;
            let rcvd_future = rx.recv();

            // Terminate serving_future by selecting another future
            let state_command = match future::select(pin!(service_future), pin!(rcvd_future)).await
            {
                future::Either::Left((state, _)) => {
                    println!("Received restart signal from the service");
                    state?
                }
                future::Either::Right((state, _)) => {
                    println!("Received restart signal from other threads");
                    state.ok_or(Error::Text("received None state from sender".to_owned()))?
                }
            };

            state = match state_command {
                StateCommand::Keep => state,
                StateCommand::Update(new) => new,
                StateCommand::BecomeScheduler(cluster) => {
                    let mean_scheduler = MeanScheduler {};
                    let scheduler = AuthoritativeScheduler::new(
                        cluster,
                        Box::new(mean_scheduler),
                        self.tx.clone(),
                        self.database.clone(),
                    );
                    DaemonState::Authoritative(scheduler)
                }
            };
        }
    }

    pub async fn start_service(
        &self,
        daemon: ServerDaemon,
        state: DaemonState,
    ) -> Result<StateCommand> {
        let server = daemon.runtime.lock().await.info.clone();

        let new_state = match state {
            DaemonState::Joining(bootstrap_addr) => {
                println!("Joining to a cluster...");
                self.join_cluster(server, &bootstrap_addr).await
            }
            DaemonState::Cloud(group) => {
                println!("Running a new server...");
                println!("group = {:?}", group);

                self.start_cloud(daemon, server, group).await
            }
            DaemonState::Fog(scheduler) => {
                println!("Running a new fog server...");

                self.start_fog(daemon, server, scheduler).await
            }
            DaemonState::Dew(scheduler) => {
                println!("Running a new dew server...");

                self.start_dew(daemon, server, scheduler).await
            }
            DaemonState::Authoritative(scheduler) => {
                println!("Running an Authoritative server...");
                self.start_authoritative(daemon, scheduler).await
            }
            DaemonState::Failed => {
                panic!("invalid state: {:?}", state)
            }
        }?;

        return Ok(StateCommand::Update(new_state));
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

        Ok(DaemonState::Cloud(group))
    }

    async fn start_authoritative(
        &self,
        daemon: ServerDaemon,
        scheduler: AuthoritativeScheduler,
    ) -> Result<DaemonState> {
        let server = daemon.runtime.lock().await.info.clone();

        let (app_tx, app_rx) = mpsc::channel(DEFAULT_STATS_CHANNELS);

        let scheduler_info = scheduler
            .runtime
            .lock()
            .await
            .cluster
            .group
            .scheduler_info
            .clone();

        let reporter_token = self.start_reporter(server.clone(), scheduler_info, app_rx);

        let grpc_server = self
            .common_services(daemon, app_tx)
            .await?
            .add_service(SchedulerServer::new(scheduler.clone()));

        println!("Listening on {}...", self.socket);
        grpc_server.serve(self.socket).await?;

        println!("cancel reporter (authoritative)");
        reporter_token.cancel();

        Ok(DaemonState::Authoritative(scheduler.clone()))
    }

    async fn start_cloud(
        &self,
        daemon: ServerDaemon,
        server: ServerInfo,
        group: GroupInfo,
    ) -> Result<DaemonState> {
        let (app_tx, app_rx) = mpsc::channel(DEFAULT_STATS_CHANNELS);

        let reporter_token =
            self.start_reporter(server.clone(), group.scheduler_info.clone(), app_rx);

        let grpc_router = self.common_services(daemon, app_tx).await?;

        grpc_router.serve(self.socket).await?;

        println!("cancel reporter (running)");
        reporter_token.cancel();

        Ok(DaemonState::Cloud(group.clone()))
    }

    async fn start_fog(
        &self,
        daemon: ServerDaemon,
        server: ServerInfo,
        scheduler: FogScheduler,
    ) -> Result<DaemonState> {
        println!("Starting fog server...");

        let (app_tx, app_rx) = mpsc::channel(DEFAULT_STATS_CHANNELS);

        let reporter_token = self.start_reporter(server.clone(), server.clone(), app_rx);

        let grpc_router = self
            .common_services(daemon, app_tx)
            .await?
            .add_service(SchedulerServer::new(scheduler.clone()));

        grpc_router.serve(self.socket).await?;

        println!("cancel reporter (running)");
        reporter_token.cancel();

        Ok(DaemonState::Fog(scheduler))
    }

    async fn start_dew(
        &self,
        daemon: ServerDaemon,
        server: ServerInfo,
        scheduler: DewScheduler,
    ) -> Result<DaemonState> {
        println!("Starting dew server...");

        let (app_tx, app_rx) = mpsc::channel(DEFAULT_STATS_CHANNELS);

        let reporter_token = self.start_reporter(server.clone(), server.clone(), app_rx);

        let grpc_router = self
            .common_services(daemon, app_tx)
            .await?
            .add_service(SchedulerServer::new(scheduler.clone()));

        grpc_router.serve(self.socket).await?;

        println!("cancel reporter (running)");
        reporter_token.cancel();

        Ok(DaemonState::Dew(scheduler))
    }

    fn start_reporter(
        &self,
        server: ServerInfo,
        scheduler: ServerInfo,
        app_rx: AppMetricReceiver,
    ) -> CancellationToken {
        let token = CancellationToken::new();
        let cloned = token.clone();

        let mut reporter = MetricsReporter::new(self.tx.clone(), app_rx, server, scheduler);
        tokio::spawn(async move { reporter.start(cloned).await });

        token
    }

    async fn common_services(
        &self,
        daemon: ServerDaemon,
        app_tx: AppMetricSender,
    ) -> Result<Router> {
        let router = TransportServer::builder()
            .add_service(ServerDaemonServer::new(daemon))
            .add_service(MiddlewareFor::new(
                GreeterServer::new(hello::MyGreeter::default()),
                MetricsMiddleware { tx: app_tx.clone() },
            ));

        #[cfg(feature = "face")]
        let router = if let Some(deployment) = self.database.lookup("face").await {
            println!("found face from db");
            use face_proto::detector_server::DetectorServer;
            use face_proto::object_detection_server::ObjectDetectionServer;
            let onnx = self
                .database
                .get(&deployment, Target::Onnx)
                .await
                .map_err(|e| format!("failed to read application binary from database: {e}"))?;

            let wasm = self
                .database
                .get(&deployment, Target::Wasm)
                .await
                .map_err(|e| format!("failed to read application binary from database: {e}"))?;

            let inner_server = FaceServer::create(onnx.clone(), wasm.clone())
                .await
                .map_err(|e| Error::AppInstantiation(e.to_string()))?;
            let server = DetectorServer::new(inner_server);
            let router = router.add_service(MiddlewareFor::new(
                server,
                MetricsMiddleware { tx: app_tx.clone() },
            ));

            let inner_server = FaceServer::create(onnx, wasm)
                .await
                .map_err(|e| Error::AppInstantiation(e.to_string()))?;
            let od_server = ObjectDetectionServer::new(inner_server);
            router.add_service(MiddlewareFor::new(
                od_server,
                MetricsMiddleware { tx: app_tx.clone() },
            ))
        } else {
            println!("not found face from db");
            router
        };

        Ok(router)
    }

    fn create_daemon(&self, info: ServerInfo, state: DaemonState) -> ServerDaemon {
        ServerDaemon::with_state(state, info, self.tx.clone(), self.database.clone())
    }

    fn create_info(&self, start_command: &StartCommand) -> Result<ServerInfo> {
        let ip = if self.socket.ip().is_unspecified() {
            local_ip()?
        } else {
            self.socket.ip()
        };

        let host = format!("{ip}:{}", self.socket.port());

        match &start_command.id {
            Some(id) => ServerInfo::with_id_str(&id, &host),
            None => Ok(ServerInfo::new(&host)),
        }
    }

    fn get_socket(start_command: &StartCommand) -> Result<SocketAddr> {
        println!("{}", &start_command.listen_host);
        Ok(SocketAddr::from_str(&start_command.listen_host)
            .map_err(|e| format!("failed to parse listen address: {e}"))?)
    }

    pub fn determine_state(
        &self,
        start_command: &StartCommand,
        server: &ServerInfo,
    ) -> DaemonState {
        match start_command.layer.as_ref() {
            "cloud" => (),
            "fog" => {
                let cloud = start_command
                    .bootstrap_addr
                    .as_ref()
                    .expect("Bootstrap server address is required on fog layer")
                    .clone();

                let mean_scheduler = Box::new(MeanScheduler {});
                let tx = self.tx.clone();

                let scheduler = FogScheduler::new(
                    server.clone(),
                    cloud,
                    mean_scheduler,
                    tx,
                    self.database.clone(),
                );

                return DaemonState::Fog(scheduler);
            }
            "dew" => {
                let parent = start_command
                    .bootstrap_addr
                    .as_ref()
                    .expect("Bootstrap server address is required on fog layer")
                    .clone();

                let mean_scheduler = Box::new(MeanScheduler {});
                let tx = self.tx.clone();

                let scheduler = DewScheduler::new(
                    server.clone(),
                    parent,
                    mean_scheduler,
                    tx,
                    self.database.clone(),
                );

                return DaemonState::Dew(scheduler);
            }
            layer => {
                panic!("Unexpected layer specification: {}", layer)
            }
        }
        let maybe_bootstrap_addr = start_command.bootstrap_addr.as_deref();

        match maybe_bootstrap_addr {
            Some(bootstrap_addr) => DaemonState::Joining(bootstrap_addr.to_owned()),
            None => {
                let inner_scheduler: Box<dyn DeploymentScheduler> =
                    match start_command.scheduler.as_str() {
                        "mean" => Box::new(MeanScheduler {}),
                        "round" => Box::new(RoundRobbinScheduler::new()),
                        otherwise => unimplemented!("Unknown scheduler algorithm: {otherwise}"),
                    };
                let tx = self.tx.clone();
                let scheduler = AuthoritativeScheduler::from_server(
                    server,
                    inner_scheduler,
                    tx,
                    self.database.clone(),
                );
                DaemonState::Authoritative(scheduler)
            }
        }
    }

    async fn scheduler_client(&self, target_addr: &str) -> Result<SchedulerClient<Channel>> {
        Ok(SchedulerClient::connect(target_addr.to_owned()).await?)
    }
}
