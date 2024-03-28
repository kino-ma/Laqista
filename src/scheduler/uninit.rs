use tonic::{Request, Response, Status};

use crate::proto::{
    scheduler_server::Scheduler, DeployRequest, DeployResponse, JoinRequest, JoinResponse,
    LookupRequest, LookupResponse, NotifyRequest, NotifyResponse,
};

#[derive(Clone, Debug)]
pub struct UninitScheduler {}

impl UninitScheduler {
    pub fn new() -> Self {
        Self {}
    }
}

#[tonic::async_trait]
impl Scheduler for UninitScheduler {
    async fn join(&self, _request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        println!("Uninit: join called!");
        Err(Status::aborted("not implemented"))
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
