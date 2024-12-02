use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use futures::TryFutureExt;
use laqista_core::{AppRpc, AppService, DeploymentInfo};
use tokio::sync::Mutex;
use tonic::{transport::Channel, Request, Response, Result as RpcResult, Status};
use uuid::Uuid;

use crate::{
    deployment::database::DeploymentDatabase,
    proto::{
        scheduler_client::SchedulerClient, scheduler_server::Scheduler,
        server_daemon_client::ServerDaemonClient, DeployRequest, DeployResponse, GetAppsRequest,
        GetAppsResponse, JoinRequest, JoinResponse, LookupRequest, LookupResponse, ReportRequest,
        ReportResponse, Server, SpawnRequest, SpawnResponse,
    },
    server::StateSender,
    utils::IdMap,
    Error, QoSSpec, Result, ServerInfo,
};

use super::{
    interface::DeploymentScheduler,
    stats::{AppLatency, ServerStats, StatsMap},
};

#[derive(Debug)]
pub struct FogScheduler {
    pub runtime: Arc<Mutex<FogSchedulerRuntime>>,
    pub tx: Arc<Mutex<StateSender>>,
}

#[derive(Clone, Debug)]
pub struct FogSchedulerRuntime {
    pub cloud_scheduler: ServerInfo,
    pub stats: ServerStats,
    pub app_stats: HashMap<AppService, AppLatency>,
    pub scheduler: Box<dyn DeploymentScheduler>,
    pub database: DeploymentDatabase,
}

impl FogScheduler {
    pub fn new(
        server: ServerInfo,
        cloud_scheduler: ServerInfo,
        scheduler: Box<dyn DeploymentScheduler>,
        tx: StateSender,
        database: DeploymentDatabase,
    ) -> Self {
        let stats = ServerStats::new(server);
        let app_stats = HashMap::new();

        let runtime = Arc::new(Mutex::new(FogSchedulerRuntime {
            cloud_scheduler,
            scheduler,
            database,
            stats,
            app_stats,
        }));

        let tx = Arc::new(Mutex::new(tx));

        Self { runtime, tx }
    }

    pub async fn deploy_in_me(&self, deployment: DeploymentInfo) -> Result<SpawnResponse> {
        let request = SpawnRequest {
            deployment: Some(deployment.clone().into()),
        };

        let this_server = &self.runtime.lock().await.stats.server;

        let mut client = self.client(this_server).await?;

        let request = Request::new(request);

        let response = client.spawn(request).await?;
        if !response.get_ref().success {
            return Err("Unsuccessful spawn".into());
        }

        println!("spawend successfully");

        Ok(response.into_inner())
    }

    pub async fn schedule_in_self(
        &self,
        request: LookupRequest,
    ) -> Result<Response<LookupResponse>> {
        let runtime = self.clone_inner().await;

        let LookupRequest {
            deployment_id,
            qos: maybe_qos,
            service,
        } = request;

        let id = Uuid::parse_str(&deployment_id).map_err(|e| Status::aborted(e.to_string()))?;
        let this_server = runtime.stats.server.clone();

        let qos: QoSSpec = maybe_qos
            .unwrap_or_default()
            .try_into()
            .map_err(<Error as Into<Status>>::into)?;

        let service = AppService::from_str(&service)
            .map_err(|_| Status::aborted(format!("failed to parse service path '{service}'")))?;

        let stats_map: StatsMap = IdMap(HashMap::from([(this_server.id, runtime.stats)]));

        let app_stats: AppLatency = runtime
            .app_stats
            .get(&service)
            .ok_or(Error::NoneError)?
            .clone();

        // 74  | pub struct AppsMap(pub HashMap<AppService, IdMap<AppLatency>>);
        let apps_map = super::stats::AppsMap(HashMap::from([(
            service.clone(),
            /* HashMap<AppService, AppLatency> */
            IdMap(HashMap::from([(this_server.id, app_stats)])),
        )]));

        let app = runtime
            .database
            .get_by_id(&id)
            .await
            .ok_or(Error::NoneError)?;

        let (target, rpc) = runtime
            .scheduler
            .schedule(&service, &app, &stats_map, &apps_map, qos)
            .ok_or(Error::NoneError)?;

        Ok(Response::new(LookupResponse {
            success: true,
            deployment_id: id.to_string(),
            server: Some(target.into()),
            rpc: rpc.to_string(),
        }))
    }

    pub async fn scheduler_client(&self, server: &ServerInfo) -> Result<SchedulerClient<Channel>> {
        Ok(SchedulerClient::connect(server.addr.clone()).await?)
    }

    pub async fn client(&self, server: &ServerInfo) -> Result<ServerDaemonClient<Channel>> {
        Ok(ServerDaemonClient::connect(server.addr.clone()).await?)
    }

    pub async fn clone_inner(&self) -> FogSchedulerRuntime {
        FogSchedulerRuntime::clone_inner(&self.runtime).await
    }
}

#[tonic::async_trait]
impl Scheduler for FogScheduler {
    async fn join(&self, _request: Request<JoinRequest>) -> RpcResult<Response<JoinResponse>> {
        unimplemented!("Fog node does not support join()");
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

        let mut runtime = FogSchedulerRuntime::clone_inner(&self.runtime).await;

        runtime.stats.append(windows);
        let info_by_name = runtime.database.list_by_names().await;
        runtime.push_latency(&server, info_by_name, app_latencies);

        self.runtime.lock().await.clone_from(&runtime);

        Ok(Response::new(ReportResponse {
            success: true,
            cluster: None,
        }))
    }

    async fn deploy(
        &self,
        _request: Request<DeployRequest>,
    ) -> RpcResult<Response<DeployResponse>> {
        unimplemented!("Fog node does not support deploy()");
    }

    async fn lookup(&self, request: Request<LookupRequest>) -> RpcResult<Response<LookupResponse>> {
        let runtime = self.clone_inner().await;

        let req = request.into_inner();

        self.schedule_in_self(req.clone())
            .or_else(|_| async {
                let mut client = self
                    .scheduler_client(&runtime.cloud_scheduler)
                    .await
                    .map_err(<Error as Into<Status>>::into)?;
                client.lookup(Request::new(req)).await
            })
            .await
    }

    async fn get_apps(
        &self,
        _request: Request<GetAppsRequest>,
    ) -> RpcResult<Response<GetAppsResponse>> {
        unimplemented!("Fog node does not support get_apps()");
    }
}

impl FogSchedulerRuntime {
    pub async fn clone_inner(arc: &Arc<Mutex<Self>>) -> Self {
        arc.lock().await.clone()
    }

    pub fn push_latency(
        &mut self,
        _server: &ServerInfo,
        info_by_name: HashMap<String, DeploymentInfo>,
        latencies: HashMap<String, u32>,
    ) {
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
                .entry(rpc.to_owned().into())
                .and_modify(|l| {
                    l.insert(&rpc, dur);
                })
                .or_insert(AppLatency::new(info.clone()));
        }
    }
}
