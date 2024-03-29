use std::fmt::Debug;

use tokio::sync::mpsc::{self, Sender};
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};

use crate::{
    proto::{
        scheduler_server::Scheduler, DeployRequest, DeployResponse, Group, JoinRequest,
        JoinResponse, LookupRequest, LookupResponse, NotifyRequest, NotifyResponse,
    },
    server::DaemonState,
    ServerInfo,
};

use super::{mean::MeanGpuScheduler, AuthoritativeScheduler};

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

    pub fn create_scheduler(&self, other: &ServerInfo) -> AuthoritativeScheduler {
        let scheduler = Box::new(MeanGpuScheduler {});
        AuthoritativeScheduler::new(&self.server, other, scheduler)
    }
}

#[tonic::async_trait]
impl Scheduler for UninitScheduler {
    async fn join(&self, request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        println!("Uninit: join called!");

        let server = request
            .into_inner()
            .server
            .ok_or(Status::aborted("Server cannot be empty"))?;
        let other: ServerInfo =
            ServerInfo::try_from(server).map_err(|e| Status::aborted(e.to_string()))?;

        let scheduler = self.create_scheduler(&other);
        let state = DaemonState::Authoritative(scheduler);

        self.tx
            .send(state)
            .await
            .map_err(|e| Status::aborted(format!("failed to send data: {}", e)))?;

        let success = true;
        let group = Some(Group {
            scheduler: Some(self.server.clone().into()),
            number: 1,
        });

        self.cancel_token.cancel();

        Ok(Response::new(JoinResponse { success, group }))
    }

    async fn notify(
        &self,
        _request: Request<NotifyRequest>,
    ) -> Result<Response<NotifyResponse>, Status> {
        println!("Uninit: notify called!");
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
