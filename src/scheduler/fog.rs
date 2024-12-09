use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use futures::TryFutureExt;
use laqista_core::{AppRpc, AppService, DeploymentInfo};
use tokio::sync::Mutex;
use tonic::{transport::Channel, Request, Response, Result as RpcResult, Status};

use crate::{
    deployment::database::DeploymentDatabase,
    proto::{
        scheduler_client::SchedulerClient, scheduler_server::Scheduler,
        server_daemon_client::ServerDaemonClient, DeployRequest, DeployResponse, GetAppsRequest,
        GetAppsResponse, GetStatsRequest, GetStatsResponse, JoinRequest, JoinResponse,
        LookupRequest, LookupResponse, NodeAppStats, RepeatedWindows, ReportRequest,
        ReportResponse, Server,
    },
    server::StateSender,
    utils::IdMap,
    Error, QoSSpec, Result, ServerInfo,
};

use super::{
    interface::DeploymentScheduler,
    stats::{AppLatency, ServerStats, StatsMap},
};

#[derive(Clone, Debug)]
pub struct FogScheduler {
    pub runtime: Arc<Mutex<FogSchedulerRuntime>>,
    pub tx: Arc<Mutex<StateSender>>,
}

#[derive(Clone, Debug)]
pub struct FogSchedulerRuntime {
    pub cloud_addr: String,
    pub stats: ServerStats,
    pub app_stats: HashMap<AppService, AppLatency>,
    pub scheduler: Box<dyn DeploymentScheduler>,
    pub database: DeploymentDatabase,
}

impl FogScheduler {
    pub fn new(
        server: ServerInfo,
        cloud_addr: String,
        scheduler: Box<dyn DeploymentScheduler>,
        tx: StateSender,
        database: DeploymentDatabase,
    ) -> Self {
        let stats = ServerStats::new(server);
        let app_stats = HashMap::new();

        let runtime = Arc::new(Mutex::new(FogSchedulerRuntime {
            cloud_addr,
            scheduler,
            database,
            stats,
            app_stats,
        }));

        let tx = Arc::new(Mutex::new(tx));

        Self { runtime, tx }
    }

    pub async fn schedule_in_self(
        &self,
        request: LookupRequest,
    ) -> Result<Response<LookupResponse>> {
        let runtime = self.clone_inner().await;

        let LookupRequest {
            qos: maybe_qos,
            service,
            name,
        } = request;

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
            .lookup(&name)
            .await
            .ok_or(Error::NoneError)?;

        let (target, rpc) = runtime
            .scheduler
            .schedule(&service, &app, &stats_map, &apps_map, qos)
            .ok_or(Error::NoneError)?;

        Ok(Response::new(LookupResponse {
            success: true,
            deployment_id: app.id.to_string(),
            server: Some(target.into()),
            rpc: rpc.to_string(),
        }))
    }

    pub async fn scheduler_client(&self, addr: String) -> Result<SchedulerClient<Channel>> {
        Ok(SchedulerClient::connect(addr).await?)
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
                    .scheduler_client(runtime.cloud_addr)
                    .await
                    .map_err(<Error as Into<Status>>::into)?;
                client.lookup(Request::new(req)).await
            })
            .await
    }

    async fn get_apps(
        &self,
        request: Request<GetAppsRequest>,
    ) -> RpcResult<Response<GetAppsResponse>> {
        let runtime = self.runtime.lock().await.clone();

        let mut client = self
            .scheduler_client(runtime.cloud_addr)
            .await
            .map_err(|e| Status::aborted(format!("failed to connect to scheduler: {e}")))?;

        let resp = client.get_apps(request).await?;

        let deployments: Vec<_> = resp
            .get_ref()
            .apps
            .iter()
            .map(|app| <DeploymentInfo as TryFrom<_>>::try_from(app.clone()).unwrap())
            .collect();

        self.runtime
            .lock()
            .await
            .database
            .add_instances(&deployments)
            .await
            .map_err(|e| Status::aborted(format!("failed to add app instances: {e}")))?;

        Ok(resp)
    }

    async fn get_stats(
        &self,
        _request: Request<GetStatsRequest>,
    ) -> RpcResult<Response<GetStatsResponse>> {
        let runtime = self.runtime.lock().await.clone();

        let server_id = runtime.stats.server.id;

        let windows = RepeatedWindows {
            windows: runtime.stats.stats,
        };
        let server_sttas = HashMap::from([(server_id.to_string(), windows)]);

        let app_stats: HashMap<String, u32> = runtime
            .app_stats
            .into_iter()
            .map(|(_svc, latency)| {
                latency
                    .rpcs
                    .into_iter()
                    .map(|(rpc, lat)| (rpc.to_string(), lat.average.as_millis() as _))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .concat()
            .into_iter()
            .collect();
        let app_latencies = HashMap::from([(server_id.to_string(), NodeAppStats { app_stats })]);

        let resp = GetStatsResponse {
            server_sttas,
            app_latencies,
        };
        Ok(Response::new(resp))
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
