use std::fmt::Debug;

use tokio::sync::mpsc::{self, Sender};
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};

use crate::{
    proto::{
        scheduler_server::Scheduler, DeployRequest, DeployResponse, JoinRequest, JoinResponse,
        LookupRequest, LookupResponse, NotifyRequest, NotifyResponse, ReportRequest,
        ReportResponse,
    },
    server::DaemonState,
    Error, ServerInfo,
};

use super::{mean::MeanGpuScheduler, AuthoritativeScheduler, Cluster};

pub struct UninitScheduler {
    server: ServerInfo,
    tx: Sender<DaemonState>,
    cancel_token: CancellationToken,
}

impl UninitScheduler {
    pub fn new(
        server: ServerInfo,
        tx: Sender<DaemonState>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            server,
            tx,
            cancel_token,
        }
    }

    pub fn create_scheduler(&self, this: Cluster, other: Cluster) -> AuthoritativeScheduler {
        let scheduler = Box::new(MeanGpuScheduler {});
        AuthoritativeScheduler::new(this, other, scheduler)
    }
}

#[tonic::async_trait]
impl Scheduler for UninitScheduler {
    async fn join(&self, request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        println!("Uninit: join called!");

        let other_server = request
            .into_inner()
            .server
            .ok_or(Status::aborted("Server cannot be empty"))?;
        let other: ServerInfo =
            ServerInfo::try_from(other_server).map_err(<Error as Into<Status>>::into)?;

        let this_cluster = Cluster::new(&self.server);
        let this_group = this_cluster.group.clone();
        let other_cluster = this_cluster.next_cluster(&other);
        let other_group = other_cluster.group.clone();
        let nomination = Some(other_cluster.to_nomination());

        let scheduler = self.create_scheduler(this_cluster, other_cluster);

        let state = DaemonState::Authoritative(scheduler);
        self.tx
            .send(state)
            .await
            .map_err(|e| Status::aborted(format!("failed to send data: {}", e)))?;

        let success = true;

        self.cancel_token.cancel();

        Ok(Response::new(JoinResponse {
            success,
            group: Some(other_group.into()),
            is_scheduler: true,
            our_group: Some(this_group.into()),
            nomination,
        }))
    }

    async fn notify(
        &self,
        _request: Request<NotifyRequest>,
    ) -> Result<Response<NotifyResponse>, Status> {
        println!("Uninit: notify called!");
        Err(Status::aborted("not implemented"))
    }

    async fn report(
        &self,
        _request: Request<ReportRequest>,
    ) -> Result<Response<ReportResponse>, Status> {
        Err(Status::aborted("not implemented"))
    }

    async fn deploy(
        &self,
        _request: Request<DeployRequest>,
    ) -> Result<Response<DeployResponse>, Status> {
        println!("Uninit: deploy called!");
        Err(Status::aborted("not implemented"))
    }

    async fn lookup(
        &self,
        _request: Request<LookupRequest>,
    ) -> Result<Response<LookupResponse>, Status> {
        println!("Uninit: lookup called!");
        Err(Status::aborted("not implemented"))
    }
}

impl Debug for UninitScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UninitScheduler")
            .field("server", &self.server)
            .field("tx", &"[Sender]")
            .finish()
    }
}

impl Clone for UninitScheduler {
    fn clone(&self) -> Self {
        println!("WARN: cloning tx");

        let server = self.server.clone();
        let (tx, _) = mpsc::channel(1);
        let cancel_token = CancellationToken::new();

        Self {
            server,
            tx,
            cancel_token,
        }
    }
}
