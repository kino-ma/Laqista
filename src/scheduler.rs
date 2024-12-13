pub mod dew;
pub mod fog;
pub mod interface;
pub mod mean;
pub mod stats;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use interface::ScheduleResult;
use laqista_core::{try_collect_accuracies, AppRpc, AppService};
use stats::{AppLatency, AppsMap};
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::deployment::database::DeploymentDatabase;
use crate::proto::scheduler_server::Scheduler;
use crate::proto::server_daemon_client::ServerDaemonClient;
use crate::proto::{
    ClusterState, DeployRequest, DeployResponse, Deployment, GetAppsRequest, GetAppsResponse,
    GetStatsRequest, GetStatsResponse, JoinRequest, JoinResponse, LookupRequest, LookupResponse,
    Nomination, RepeatedWindows, ReportRequest, ReportResponse, Server, SpawnRequest,
    SpawnResponse,
};
use crate::server::{DaemonState, StateSender};
use crate::utils::IdMap;
use crate::{
    AppInstanceMap, AppInstancesInfo, DeploymentInfo, GroupInfo, QoSSpec, RpcResult, ServerInfo,
};
use crate::{Error, Result};

use self::interface::DeploymentScheduler;
use self::stats::{ServerStats, StatsMap};

#[derive(Debug)]
pub struct AuthoritativeScheduler {
    pub runtime: Arc<Mutex<SchedulerRuntime>>,
    pub tx: Arc<Mutex<StateSender>>,
}

#[derive(Clone, Debug)]
pub struct SchedulerRuntime {
    pub cluster: Cluster,
    pub scheduler: Box<dyn DeploymentScheduler>,
    pub deployments: IdMap<DeploymentInfo>,
    pub database: DeploymentDatabase,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    pub group: GroupInfo,
    pub servers: Vec<ServerInfo>,
    pub instances: AppInstanceMap,
    pub server_stats: StatsMap,
    pub app_stats: AppsMap,
}

impl AuthoritativeScheduler {
    pub fn new(
        cluster: Cluster,
        scheduler: Box<dyn DeploymentScheduler>,
        tx: StateSender,
        database: DeploymentDatabase,
    ) -> Self {
        let runtime = Arc::new(Mutex::new(SchedulerRuntime {
            cluster,
            scheduler,
            deployments: IdMap::new(),
            database,
        }));

        let tx = Arc::new(Mutex::new(tx));

        Self { runtime, tx }
    }

    pub fn from_server(
        server: &ServerInfo,
        scheduler: Box<dyn DeploymentScheduler>,
        tx: StateSender,
        database: DeploymentDatabase,
    ) -> Self {
        let cluster = Cluster::new(server);

        Self::new(cluster, scheduler, tx, database)
    }

    pub async fn push_server(&self, server: ServerInfo) {
        self.runtime.lock().await.cluster.servers.push(server);
    }

    pub async fn deploy_in_us(&self, deployment: DeploymentInfo) -> Result<SpawnResponse> {
        let request = SpawnRequest {
            deployment: Some(deployment.clone().into()),
        };

        let target_server = {
            let runtime = self.runtime.lock().await;
            runtime
                .scheduler
                .least_utilized(&runtime.cluster.server_stats)
        };

        println!("Scaling out the application: {deployment:?} to {target_server:?}");

        let mut client = self.client(&target_server).await?;

        let request = Request::new(request);

        let response = client.spawn(request).await?;
        if !response.get_ref().success {
            return Err("Unsuccessful spawn".into());
        }

        println!("spawend successfully");

        // Update deployments information and instances information atomicly
        {
            println!("taking lock of runtime");
            let mut runtime = self.runtime.lock().await;
            runtime
                .deployments
                .0
                .insert(deployment.id, deployment.clone());

            runtime
                .cluster
                .insert_instance(deployment, vec![target_server]);
            println!("freeing runtime");
        }

        println!(
            "Successfully spawned app ({:?}) on {:?}",
            response.get_ref().deployment,
            response.get_ref().server,
        );

        Ok(response.into_inner())
    }

    pub async fn handle_failed_server<T>(
        &self,
        result: Result<T>,
        server: &Server,
    ) -> RpcResult<Option<DaemonState>> {
        let e = match result {
            Err(Error::TransportError(e)) => e,
            Err(e) => return Err(e.into()),
            _ => return Ok(None),
        };

        println!(
            "Server {:?} has failed with following error: {e:?}",
            server.id
        );

        let id =
            Uuid::try_parse(&server.id).map_err(|e| <Error as Into<Status>>::into(e.into()))?;

        let mut runtime = self.runtime.lock().await;

        let maybe_removed = runtime.cluster.remove_server(&id);
        if maybe_removed.is_none() {
            println!("WARN: failed to remove the server from list: {server:?}");
        }

        Ok(None)
    }

    pub async fn client(&self, server: &ServerInfo) -> Result<ServerDaemonClient<Channel>> {
        Ok(ServerDaemonClient::connect(server.addr.clone()).await?)
    }

    pub async fn clone_inner(&self) -> SchedulerRuntime {
        SchedulerRuntime::clone_inner(&self.runtime).await
    }
}

#[tonic::async_trait]
impl Scheduler for AuthoritativeScheduler {
    async fn join(&self, request: Request<JoinRequest>) -> RpcResult<Response<JoinResponse>> {
        println!("join() called!!");

        let proto_server = request
            .get_ref()
            .server
            .clone()
            .expect("server cannot be None");

        let server: ServerInfo = proto_server
            .clone()
            .try_into()
            .map_err(<Error as Into<Status>>::into)?;

        self.push_server(server).await;

        let group = Some(self.runtime.lock().await.cluster.group.clone().into());

        Ok(Response::new(JoinResponse {
            success: true,
            group,
        }))
    }

    async fn report(&self, request: Request<ReportRequest>) -> RpcResult<Response<ReportResponse>> {
        let ReportRequest {
            server,
            windows,
            app_latencies,
        } = request.into_inner();

        let server: Server = server.ok_or(Status::aborted("server cannot be empty"))?;
        let server = ServerInfo::try_from(server);
        let server = server.map_err(|e| <Error as Into<Status>>::into(e))?;

        let stats = ServerStats::from_stats(server.clone(), windows);

        let mut runtime = SchedulerRuntime::clone_inner(&self.runtime).await;

        runtime.cluster.insert_stats(stats);
        let info_by_name = runtime.database.list_by_names().await;
        runtime
            .cluster
            .push_latency(&server, info_by_name, app_latencies);

        let cluster = runtime.cluster.clone().into();

        self.runtime.lock().await.clone_from(runtime);

        Ok(Response::new(ReportResponse {
            success: true,
            cluster: Some(cluster),
        }))
    }

    async fn deploy(&self, request: Request<DeployRequest>) -> RpcResult<Response<DeployResponse>> {
        println!("deploy() called!!");

        let DeployRequest {
            name,
            source,
            accuracies_percent,
            rpcs,
        } = request.into_inner();

        let accuracies = try_collect_accuracies(accuracies_percent)
            .ok_or(Status::aborted("failed to parse rpc path"))?;

        let deployment_info = DeploymentInfo::from_rpcs(name, source.clone(), &rpcs, accuracies)
            .ok_or(Status::aborted("failed to parse rpc names"))?;
        let deployment: Deployment = deployment_info.clone().into();
        println!("created info");

        self.clone_inner()
            .await
            .save_deployment(&deployment_info)
            .await
            .map_err(<Error as Into<Status>>::into)?;

        let mut success = true;

        let resp = self.deploy_in_us(deployment_info).await;
        let resp = resp.map_err(<Error as Into<Status>>::into)?;
        success &= resp.success;
        println!("got resp");

        Ok(Response::new(DeployResponse {
            success,
            deployment: Some(deployment),
        }))
    }

    async fn lookup(&self, request: Request<LookupRequest>) -> RpcResult<Response<LookupResponse>> {
        let runtime = self.clone_inner().await;

        let LookupRequest {
            qos: maybe_qos,
            service,
            name,
        } = request.into_inner();

        let app = runtime
            .get_deployment(&name)
            .ok_or(Status::aborted(format!(
                "Application with that name not found: {}",
                name
            )))?;
        let id = app.id;

        let qos: QoSSpec = maybe_qos
            .unwrap_or_default()
            .try_into()
            .map_err(<Error as Into<Status>>::into)?;

        let server_ids = runtime
            .cluster
            .get_instance_server_ids(&id)
            .map_err(|e| Status::aborted(e.to_string()))?;

        let stats_map = runtime.cluster.server_stats.clone_by_ids(&server_ids);
        let apps_map = runtime.cluster.app_stats.clone();

        let service = AppService::from_str(&service)
            .map_err(|_| Status::aborted(format!("failed to parse service path '{service}'")))?;

        let ScheduleResult {
            server: target,
            rpc,
            needs_scale_out,
        } = runtime
            .scheduler
            .schedule(&service, &app, &stats_map, &apps_map, qos)
            .or_else(|| {
                let instance = runtime.cluster.instances.0.get(&id)?;
                let server = instance.servers.get(0)?;
                let rpc = app.services.get(&service).unwrap().get(0)?;
                let res = ScheduleResult::new(server.clone(), rpc.clone(), true);
                Some(res)
            })
            .ok_or(Status::aborted("Failed to schedule: No server found"))?
            .clone();

        if needs_scale_out {
            // Clone self.
            // Because we have Arc<Mutex<_>> inside Self, we can edit the inner data from the clone.
            let this = self.clone();
            tokio::task::spawn(async move {
                let deployment = this
                    .runtime
                    .lock()
                    .await
                    .deployments
                    .0
                    .get(&id)
                    .ok_or(())?
                    .clone();

                this.deploy_in_us(deployment)
                    .await
                    .err()
                    .map(|e| println!("ERR: deploy_in_us failed: {e}"));

                Ok::<(), ()>(())
            });
        }

        Ok(Response::new(LookupResponse {
            success: true,
            deployment_id: id.to_string(),
            server: Some(target.into()),
            rpc: rpc.to_string(),
        }))
    }

    async fn get_apps(
        &self,
        request: Request<GetAppsRequest>,
    ) -> RpcResult<Response<GetAppsResponse>> {
        let names = request.into_inner().names;

        let runtime = self.runtime.lock().await.clone();
        let apps = runtime
            .deployments
            .0
            .into_iter()
            .filter(|(_, v)| names.contains(&v.name))
            .map(|(_, v)| v.into())
            .collect();

        let resp = GetAppsResponse { apps };

        Ok(Response::new(resp))
    }

    async fn get_stats(
        &self,
        _request: Request<GetStatsRequest>,
    ) -> RpcResult<Response<GetStatsResponse>> {
        let runtime = self.runtime.lock().await.clone();

        let server_sttas: HashMap<String, RepeatedWindows> = runtime
            .cluster
            .server_stats
            .0
            .into_iter()
            .map(|(id, stats)| {
                let windows = RepeatedWindows {
                    windows: stats.stats,
                };
                (id.to_string(), windows)
            })
            .collect();

        let app_latencies = runtime.cluster.app_stats.into_node_stats();

        let resp = GetStatsResponse {
            server_sttas,
            app_latencies,
        };
        Ok(Response::new(resp))
    }
}

impl Clone for AuthoritativeScheduler {
    fn clone(&self) -> Self {
        let runtime = self.runtime.clone();
        let tx = self.tx.clone();

        Self {
            runtime: runtime,
            tx,
        }
    }
}

impl SchedulerRuntime {
    pub fn new(
        this_server: &ServerInfo,
        scheduler: Box<dyn DeploymentScheduler>,
        database: DeploymentDatabase,
    ) -> Self {
        let cluster = Cluster::new(this_server);

        Self {
            cluster,
            scheduler,
            deployments: IdMap::new(),
            database,
        }
    }

    pub async fn save_deployment(&mut self, deployment: &DeploymentInfo) -> Result<()> {
        self.database
            .add_app(deployment)
            .await
            .map_err(|e| format!("Failed to save deployment: {e}"))?;

        Ok(())
    }

    pub fn get_deployment(&self, name: &str) -> Option<&DeploymentInfo> {
        self.deployments
            .iter()
            .find(|(_, d)| d.name == name)
            .map(|(_, d)| d)
    }

    pub fn wrap(self) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(self))
    }

    pub async fn clone_inner(arc: &Arc<Mutex<Self>>) -> Self {
        arc.lock().await.clone()
    }

    pub fn clone_from(&mut self, other: Self) {
        self.cluster = other.cluster;
        self.scheduler = other.scheduler;
        self.deployments = other.deployments;
        self.database = other.database;
    }
}

impl Cluster {
    pub fn new(scheduler: &ServerInfo) -> Self {
        let group = GroupInfo::new(scheduler);
        let servers = vec![scheduler.clone()];
        let instances = AppInstanceMap::new();
        let server_stats = StatsMap::new();
        let app_stats = AppsMap::new();

        Self {
            group,
            servers,
            instances,
            server_stats,
            app_stats,
        }
    }

    pub fn with_group(group: &GroupInfo) -> Self {
        let group = group.clone();
        let servers = vec![group.scheduler_info.clone()];
        let instances = AppInstanceMap::new();
        let server_stats = StatsMap::new();
        let app_stats = AppsMap::new();

        Self {
            group,
            servers,
            instances,
            server_stats,
            app_stats,
        }
    }

    /// choose_scheduler chooses a scheduler based on their ids.
    /// A server with the lowest id is chosen.
    ///
    /// NOTE:
    /// This function assumes there is at least one server in the `.servers`
    /// becouse the calling server instance must be included.
    pub fn choose_scheduler(&mut self) -> &ServerInfo {
        let mut lowest_id_server = &self.servers[0];

        for server in &self.servers {
            if server.id < lowest_id_server.id {
                lowest_id_server = server;
            }
        }

        self.group.scheduler_info = lowest_id_server.clone();
        return &lowest_id_server;
    }

    pub fn next_cluster(&self, scheduler_info: &ServerInfo) -> Self {
        let number = self.group.number + 1;
        let other_group = GroupInfo::with_number(scheduler_info, number);
        Self::with_group(&other_group)
    }

    pub fn to_nomination(&self) -> Nomination {
        let cluster = Some(self.clone().into());
        Nomination { cluster }
    }

    pub fn insert_instance(&mut self, deployment: DeploymentInfo, servers: Vec<ServerInfo>) {
        let id = deployment.id;
        self.instances
            .0
            .entry(id)
            .and_modify(|i| i.servers.append(&mut servers.clone()))
            .or_insert(AppInstancesInfo {
                deployment,
                servers,
            });
    }

    pub fn insert_stats(&mut self, stats: ServerStats) {
        let id = stats.server.id;
        self.server_stats
            .0
            .entry(id)
            .and_modify(|s| s.append(stats.stats.clone()))
            .or_insert_with(|| {
                println!(
                    "Received server stats from {:?} for the first time",
                    &stats.server
                );
                stats
            });
    }

    pub fn push_latency(
        &mut self,
        server: &ServerInfo,
        info_by_name: HashMap<String, DeploymentInfo>,
        latencies: HashMap<String, u32>,
    ) {
        // dbg!(&info_by_name);
        let cloned_latencies = latencies.clone();

        for (path, elapsed) in latencies {
            let rpc = AppRpc::from_str(&path)
                .expect(("failed to parse gRPC path".to_owned() + &path).as_str());

            let dur = Duration::from_millis(elapsed as _);

            let service: AppService = rpc.clone().into();
            let info = &info_by_name
                .get(&rpc.package)
                .expect(&format!(
                    "failed to get key '{}' from:\n{:?}",
                    service.to_string(),
                    info_by_name
                ))
                .to_owned();

            self.app_stats
                .0
                .entry(rpc.to_owned().into())
                .and_modify(|e| {
                    e.0.entry(server.id)
                        .and_modify(|latency| latency.insert(&rpc, dur));
                })
                .or_insert_with(|| {
                    println!(
                        "Received app stats from {:?} for the first time: {:?}",
                        &server, &cloned_latencies
                    );
                    let mut app_latency = AppLatency::new(info.clone());
                    app_latency.insert(&rpc, Duration::from_millis(elapsed as _));
                    IdMap(HashMap::from([(server.id, app_latency)]))
                });
        }
    }

    pub fn remove_server(&mut self, id: &Uuid) -> Option<ServerInfo> {
        let index = self.servers.iter().position(|s| &s.id == id)?;
        Some(self.servers.remove(index))
    }

    pub fn get_instance_server_ids(&self, deployment_id: &Uuid) -> Result<Vec<Uuid>> {
        Ok(self
            .instances
            .0
            .get(deployment_id)
            .ok_or("Deployment not found".to_string())?
            .servers
            .iter()
            .map(|s| s.id)
            .collect())
    }

    pub fn get_addr(&self) -> &str {
        &self.group.scheduler_info.addr
    }
}

impl Into<ClusterState> for Cluster {
    fn into(self) -> ClusterState {
        let group = Some(self.group.into());
        let servers = self.servers.iter().map(|s| s.clone().into()).collect();
        let instances = self
            .instances
            .0
            .values()
            .map(|i| i.clone().into())
            .collect();

        ClusterState {
            group,
            servers,
            instances,
        }
    }
}

impl TryFrom<ClusterState> for Cluster {
    type Error = Error;
    fn try_from(state: ClusterState) -> Result<Self> {
        let group = state
            .group
            .ok_or("Group cannot be empty".to_owned())?
            .try_into()?;

        let servers = state
            .servers
            .into_iter()
            .map(ServerInfo::try_from)
            .collect::<Result<_>>()?;

        let instances = state
            .instances
            .into_iter()
            .map(|i| AppInstancesInfo::try_from(i).map(|ii| (ii.deployment.id, ii)))
            .collect::<Result<HashMap<_, _, _>>>()
            .map(IdMap)?;

        let server_stats = IdMap::new();
        let app_stats = AppsMap::new();

        Ok(Self {
            group,
            servers,
            instances,
            server_stats,
            app_stats,
        })
    }
}
