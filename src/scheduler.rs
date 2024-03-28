mod interface;
mod mean;
mod stats;

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::error::Error;

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

pub struct SchedulerRuntime {
    cluster: Cluster,
    other: Cluster,
    scheduler: Box<dyn DeploymentScheduler>,
    deployments: IdMap<DeploymentInfo>,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    group: GroupInfo,
    servers: Vec<ServerInfo>,
    instances: AppInstanceMap,
    server_stats: StatsMap,
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

    pub fn push_server(&self, server: ServerInfo) {
        self.cluster.servers.push(server);
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

        let cluster = Some(self.cluster.clone().try_into()?);
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
        let target_server = self
            .scheduler
            .schedule(&self.cluster.server_stats)
            .ok_or(Status::new(Code::Aborted, "failed to schedule new job"))?;

        let mut client = self.client(target_server).await?;

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
        let other_addr = self.other.get_addr().to_owned();

        return Ok(SchedulerClient::connect(other_addr).await?);
    }
}

#[tonic::async_trait]
impl Scheduler for SchedulerRuntime {
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

        Ok(Response::new(JoinResponse { success: true }))
    }

    async fn notify(
        &self,
        request: Request<NotifyRequest>,
    ) -> Result<Response<NotifyResponse>, Status> {
        println!("notify() called!!");

        let NotifyRequest { cluster: state } = request.into_inner();
        let state = state.expect("State cannot be None");

        let mut mut_self = self.borrow_mut();
        mut_self.other = state.try_into().map_err(Status::aborted)?;

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

        let deployment_info = DeploymentInfo::new(source);
        let deployment: Deployment = deployment_info.clone().into();

        let mut success = true;

        let resp = self
            .deploy_in_us(SpawnRequest {
                deployment: Some(deployment.clone()),
            })
            .await
            .map_err(|e| Status::aborted(e.to_string()))?;

        self.deployments
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
        let id = Uuid::parse_str(&request.get_ref().deployment_id)
            .map_err(|e| Status::aborted(e.to_string()))?;

        let deployment = self
            .deployments
            .0
            .get(&id)
            .ok_or(Status::aborted("Deployment not found"))?;

        let server_ids = self
            .cluster
            .get_instance_server_ids(&id)
            .map_err(Status::aborted)?;

        let stats_map = self.cluster.server_stats.clone_by_ids(server_ids);

        let target = self
            .scheduler
            .schedule(&stats_map)
            .ok_or(Status::aborted("Failed to schedule"))?;

        Ok(Response::new(LookupResponse {
            success: true,
            deployment_id: id.to_string(),
            server: Some((*target).into()),
        }))
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

    pub fn get_instance_server_ids(&self, deployment_id: &Uuid) -> Result<&[Uuid], String> {
        Ok(&self
            .instances
            .0
            .get(deployment_id)
            .ok_or("Deployment not found".to_string())?
            .servers
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>())
    }

    pub fn get_addr(&self) -> &str {
        &self.group.scheduler_info.addr
    }
}

impl Into<ClusterState> for Cluster {
    fn into(self) -> ClusterState {
        let group = Some(self.group.into());
        let servers = self.servers.iter().map(|s| (*s).into()).collect();
        let instances = self.instances.0.values().map(|i| (*i).into()).collect();

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
            .filter_map(|i| {
                i.deployment
                    .map(|d| AppInstancesInfo::try_from(i).map(|ii| (ii.deployment.id, ii)))
            })
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
