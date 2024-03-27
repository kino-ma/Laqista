mod interface;
mod mean;
mod stats;

use std::borrow::BorrowMut;
use std::error::Error;

use tonic::transport::Channel;
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use crate::proto::scheduler_client::SchedulerClient;
use crate::proto::scheduler_server::{Scheduler, SchedulerServer};
use crate::proto::server_daemon_client::ServerDaemonClient;
use crate::proto::{
    ClusterState, DeployRequest, DeployResponse, Deployment, JoinRequest, JoinResponse,
    LookupRequest, LookupResponse, MonitorResponse, MonitorWindow, NotifyRequest, NotifyResponse,
    SpawnRequest, SpawnResponse,
};
use crate::{DeploymentInfo, ServerInfo};

use self::interface::DeploymentScheduler;
use self::mean::MeanGpuScheduler;
use self::stats::StatsMap;

pub struct SchedulerRuntime {
    cluster: Cluster,
    other: Cluster,
    scheduler: Box<dyn DeploymentScheduler>,
    deployments: Vec<DeploymentInfo>,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    state: ClusterState,
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
            deployments: vec![],
        }
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
        let request = Request::new(NotifyRequest {
            cluster: Some(self.cluster.state.clone()),
        });

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

        let JoinRequest { server } = request.get_ref();
        let server = server.expect("server cannot be None");
        self.cluster.state.servers.push(server);

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
        mut_self.other.state = state;

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

        self.deployments.push(deployment_info);
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
}

impl Cluster {
    pub fn new(server: &ServerInfo) -> Self {
        let state = ClusterState {
            group: None,
            servers: vec![],
            instances: vec![],
        };

        let server_stats = StatsMap::new();

        Self {
            state,
            server_stats,
        }
    }

    pub fn get_addr(&self) -> &str {
        &self
            .state
            .group
            .expect("Group cannot be empty")
            .scheduler
            .expect("Scheduler cannot be empty")
            .addr
    }
}
