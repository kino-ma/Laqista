pub mod interface;
pub mod mean;
pub mod stats;
pub mod uninit;

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};

use tonic::transport::Channel;
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use crate::proto::scheduler_client::SchedulerClient;
use crate::proto::scheduler_server::Scheduler;
use crate::proto::server_daemon_client::ServerDaemonClient;
use crate::proto::{
    ClusterState, DeployRequest, DeployResponse, Deployment, JoinRequest, JoinResponse,
    LookupRequest, LookupResponse, NotifyRequest, NotifyResponse, SpawnRequest, SpawnResponse,
};
use crate::utils::IdMap;
use crate::{AppInstanceMap, AppInstancesInfo, DeploymentInfo, GroupInfo, ServerInfo};

use self::interface::DeploymentScheduler;
use self::stats::StatsMap;

#[derive(Debug)]
pub struct AuthoritativeScheduler {
    pub runtime: Arc<Mutex<SchedulerRuntime>>,
}

#[derive(Clone, Debug)]
pub struct SchedulerRuntime {
    pub cluster: Cluster,
    pub other: Cluster,
    pub scheduler: Box<dyn DeploymentScheduler>,
    pub deployments: IdMap<DeploymentInfo>,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    pub group: GroupInfo,
    pub servers: Vec<ServerInfo>,
    pub instances: AppInstanceMap,
    pub server_stats: StatsMap,
}

impl AuthoritativeScheduler {
    pub fn new(
        this_server: &ServerInfo,
        other_server: &ServerInfo,
        scheduler: Box<dyn DeploymentScheduler>,
    ) -> Self {
        let cluster = Cluster::new(this_server);
        let other = Cluster::new(other_server);

        let runtime = Arc::new(Mutex::new(SchedulerRuntime {
            cluster,
            other,
            scheduler,
            deployments: IdMap::new(),
        }));

        Self { runtime }
    }

    pub fn push_server(&self, server: ServerInfo) {
        self.runtime.lock().unwrap().cluster.servers.push(server);
    }

    pub async fn deploy_to_other(
        &self,
        request: DeployRequest,
    ) -> Result<DeployResponse, Box<dyn Error>> {
        let mut client = self.other_client().await?;
        let request = Request::new(request);

        let response = client.deploy(request).await?;
        if !response.get_ref().success {
            return Err("Unsuccessful deploy".into());
        }

        println!(
            "Successfully deployed to the other group: {:?}",
            response.get_ref().deployment
        );

        Ok(response.into_inner())
    }

    pub async fn notify_to_other(&self) -> Result<(), Box<dyn Error>> {
        let mut client = self.other_client().await?;

        let runtime = SchedulerRuntime::clone_inner(&self.runtime);
        let cluster = Some(runtime.cluster.try_into()?);
        let request = Request::new(NotifyRequest { cluster });

        let response = client.notify(request).await?;
        if !response.get_ref().success {
            return Err("Unsuccessful notify".into());
        }

        println!("Successfully notified to the other group");

        Ok(())
    }

    pub async fn deploy_in_us(
        &self,
        request: SpawnRequest,
    ) -> Result<SpawnResponse, Box<dyn Error>> {
        let runtime = self.clone_inner();
        let target_server = runtime
            .scheduler
            .schedule(&runtime.cluster.server_stats)
            .ok_or(Status::new(Code::Aborted, "failed to schedule new job"))?;

        let mut client = self.client(&target_server).await?;

        let request = Request::new(request);

        let response = client.spawn(request).await?;
        if !response.get_ref().success {
            return Err("Unsuccessful spawn".into());
        }

        println!(
            "Successfully spawned app ({:?}) on {:?}",
            response.get_ref().deployment,
            response.get_ref().server,
        );

        Ok(response.into_inner())
    }

    pub async fn client(
        &self,
        server: &ServerInfo,
    ) -> Result<ServerDaemonClient<Channel>, Box<dyn Error>> {
        Ok(ServerDaemonClient::connect(server.addr.clone()).await?)
    }

    pub async fn other_client(&self) -> Result<SchedulerClient<Channel>, Box<dyn Error>> {
        let other_addr = self.runtime.lock().unwrap().other.get_addr().to_owned();

        return Ok(SchedulerClient::connect(other_addr).await?);
    }

    pub fn clone_inner(&self) -> SchedulerRuntime {
        SchedulerRuntime::clone_inner(&self.runtime)
    }
}

#[tonic::async_trait]
impl Scheduler for AuthoritativeScheduler {
    async fn join(&self, request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        println!("join() called!!");

        let server = request
            .get_ref()
            .server
            .clone()
            .expect("server cannot be None")
            .try_into()
            .map_err(|e: uuid::Error| Status::aborted(e.to_string()))?;

        self.push_server(server);

        self.notify_to_other()
            .await
            .map_err(|e| Status::new(Code::Aborted, e.to_string()))?;

        let group = Some(self.runtime.lock().unwrap().cluster.group.clone().into());

        Ok(Response::new(JoinResponse {
            success: true,
            group,
        }))
    }

    async fn notify(
        &self,
        request: Request<NotifyRequest>,
    ) -> Result<Response<NotifyResponse>, Status> {
        println!("notify() called!!");

        let NotifyRequest { cluster: state } = request.into_inner();
        let state = state.expect("State cannot be None");

        let mut lock = self.runtime.lock().unwrap();
        let runtime = lock.borrow_mut();
        runtime.other = state.try_into().map_err(Status::aborted)?;

        Ok(Response::new(NotifyResponse { success: true }))
    }

    async fn deploy(
        &self,
        request: Request<DeployRequest>,
    ) -> Result<Response<DeployResponse>, Status> {
        println!("deploy() called!!");

        let DeployRequest {
            source,
            authoritative,
        } = request.into_inner();

        let deployment_info = DeploymentInfo::new(source.clone());
        let deployment: Deployment = deployment_info.clone().into();

        let mut success = true;

        let resp = self
            .deploy_in_us(SpawnRequest {
                deployment: Some(deployment.clone()),
            })
            .await
            .map_err(|e| Status::aborted(e.to_string()))?;

        self.runtime
            .lock()
            .unwrap()
            .deployments
            .0
            .insert(deployment_info.id, deployment_info);
        success &= resp.success;

        if authoritative {
            let resp = self
                .deploy_to_other(DeployRequest {
                    source,
                    authoritative: false,
                })
                .await
                .map_err(|e| Status::aborted(e.to_string()))?;

            success &= resp.success;
        }

        Ok(Response::new(DeployResponse {
            success,
            deployment: Some(deployment),
        }))
    }

    async fn lookup(
        &self,
        request: Request<LookupRequest>,
    ) -> Result<Response<LookupResponse>, Status> {
        let runtime = self.clone_inner();

        let id = Uuid::parse_str(&request.get_ref().deployment_id)
            .map_err(|e| Status::aborted(e.to_string()))?;

        let server_ids = runtime
            .cluster
            .get_instance_server_ids(&id)
            .map_err(Status::aborted)?;

        let stats_map = runtime.cluster.server_stats.clone_by_ids(&server_ids);

        let target = runtime
            .scheduler
            .schedule(&stats_map)
            .ok_or(Status::aborted("Failed to schedule"))?
            .clone();

        Ok(Response::new(LookupResponse {
            success: true,
            deployment_id: id.to_string(),
            server: Some(target.into()),
        }))
    }
}

impl Clone for AuthoritativeScheduler {
    fn clone(&self) -> Self {
        let runtime = SchedulerRuntime::clone_inner(&self.runtime);
        Self {
            runtime: Arc::new(Mutex::new(runtime)),
        }
    }
}

impl SchedulerRuntime {
    pub fn new(
        this_server: &ServerInfo,
        other_server: &ServerInfo,
        scheduler: Box<dyn DeploymentScheduler>,
    ) -> Self {
        let cluster = Cluster::new(this_server);
        let other = Cluster::new(other_server);

        Self {
            cluster,
            other,
            scheduler,
            deployments: IdMap::new(),
        }
    }

    pub fn wrap(self) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(self))
    }

    pub fn clone_inner(arc: &Arc<Mutex<Self>>) -> Self {
        arc.lock().unwrap().clone()
    }
}

impl Cluster {
    pub fn new(scheduler: &ServerInfo) -> Self {
        let group = GroupInfo::new(scheduler);
        let servers = vec![];
        let instances = AppInstanceMap::new();
        let server_stats = StatsMap::new();

        Self {
            group,
            servers,
            instances,
            server_stats,
        }
    }

    pub fn get_instance_server_ids(&self, deployment_id: &Uuid) -> Result<Vec<Uuid>, String> {
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
    type Error = String;
    fn try_from(state: ClusterState) -> Result<Self, Self::Error> {
        let group = state
            .group
            .ok_or("Group cannot be empty".to_owned())?
            .try_into()?;

        let servers = state
            .servers
            .into_iter()
            .map(ServerInfo::try_from)
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;

        let instances = state
            .instances
            .into_iter()
            .map(|i| AppInstancesInfo::try_from(i).map(|ii| (ii.deployment.id, ii)))
            .collect::<Result<HashMap<_, _, _>, _>>()
            .map(IdMap)?;

        let server_stats = IdMap::new();

        Ok(Self {
            group,
            servers,
            instances,
            server_stats,
        })
    }
}
