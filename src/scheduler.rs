use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::error::Error;

use tonic::transport::Channel;
use tonic::{transport::Server, Code, Request, Response, Status};
use uuid::Uuid;

use crate::proto::scheduler_client::SchedulerClient;
use crate::proto::scheduler_server::{Scheduler, SchedulerServer};
use crate::proto::{
    ClusterState, DeployRequest, DeployResponse, JoinRequest, JoinResponse, LookupRequest,
    LookupResponse, MonitorResponse, MonitorWindow, NotifyRequest, NotifyResponse,
};
use crate::{GroupInfo, ServerInfo};

#[derive(Clone, Debug)]
pub struct SchedulerRuntime {
    cluster: Cluster,
    other: Cluster,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    state: ClusterState,
    server_stats: StatsMap,
}

pub type StatsMap = HashMap<Uuid, ServerStats>;

#[derive(Clone, Debug)]
pub struct ServerStats {
    server: Server,
    stats: Vec<MonitorWindow>,
}

impl SchedulerRuntime {
    pub fn new(this_server: &ServerInfo, other_server: &ServerInfo) -> Self {
        let cluster = Cluster::new(this_server);
        let other = Cluster::new(other_server);

        Self { cluster, other }
    }

    pub async fn notify_to_other(&self) -> Result<(), Box<dyn Error>> {
        let mut client = self.client().await?;
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

    pub async fn client(&self) -> Result<SchedulerClient<Channel>, Box<dyn Error>> {
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
