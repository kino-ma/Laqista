use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tonic::Status;
use tonic::{Request, Response};
use uuid::Uuid;

use crate::deployment::database::DeploymentDatabase;
use crate::proto::server_daemon_server::ServerDaemon as ServerDaemonTrait;
use crate::proto::{
    DestroyRequest, DestroyResponse, GetInfoRequest, GetInfoResponse, MonitorRequest,
    MonitorResponse, NominateRequest, NominateResponse, PingResponse, ServerState, SpawnRequest,
    SpawnResponse,
};
use crate::{RpcResult, ServerInfo};

use super::{DaemonState, DEFAULT_HOST};

#[derive(Clone, Debug)]
pub struct ServerDaemon {
    pub runtime: Arc<Mutex<ServerDaemonRuntime>>,
    pub tx: mpsc::Sender<DaemonState>,
    pub state: DaemonState,
}

#[derive(Clone, Debug)]
pub struct ServerDaemonRuntime {
    pub info: ServerInfo,
    pub database: DeploymentDatabase,
}

impl ServerDaemon {
    pub fn with_state(state: DaemonState, info: ServerInfo, tx: mpsc::Sender<DaemonState>) -> Self {
        let database = DeploymentDatabase::default();
        let runtime = Arc::new(Mutex::new(ServerDaemonRuntime { info, database }));

        Self { runtime, tx, state }
    }
}

#[tonic::async_trait]
impl ServerDaemonTrait for ServerDaemon {
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> RpcResult<Response<GetInfoResponse>> {
        println!("GetInfo called!");

        let server = Some(self.runtime.lock().await.info.clone().into());
        let state = &self.state;

        use DaemonState::*;
        let group = match &state {
            Failed | Joining(_) => None,
            Running(group) => Some(group.clone().into()),
            Authoritative(scheduler) => {
                Some(scheduler.runtime.lock().await.cluster.group.clone().into())
            }
        };

        let state: ServerState = state.clone().into();
        let state = state.into();

        let resposne = GetInfoResponse {
            server,
            group,
            state,
        };

        Ok(Response::new(resposne))
    }

    async fn ping(&self, _request: Request<()>) -> RpcResult<Response<PingResponse>> {
        println!("got ping!");

        let resposne = PingResponse { success: true };

        Ok(Response::new(resposne))
    }

    async fn nominate(
        &self,
        _request: Request<NominateRequest>,
    ) -> RpcResult<Response<NominateResponse>> {
        println!("got nominate!");
        Err(Status::unimplemented("not implemented"))
    }

    async fn monitor(
        &self,
        _request: Request<MonitorRequest>,
    ) -> RpcResult<Response<MonitorResponse>> {
        Ok(Response::new(MonitorResponse { windows: vec![] }))
    }
    async fn spawn(&self, request: Request<SpawnRequest>) -> RpcResult<Response<SpawnResponse>> {
        let deployment = request
            .into_inner()
            .deployment
            .ok_or(Status::aborted("`deployment` is required`"))?;

        let id = Uuid::try_parse(&deployment.id)
            .map_err(|e| Status::aborted(format!("failed to parse uuid: {e}")))?;

        self.runtime
            .lock()
            .await
            .database
            .insert(id, deployment.source)
            .await
            .map_err(|e| {
                Status::aborted(format!("failed to insert deployment into database: {e}"))
            })?;

        Ok(Response::new(SpawnResponse {
            success: true,
            deployment: None,
            server: None,
        }))
    }
    async fn destroy(
        &self,
        _request: Request<DestroyRequest>,
    ) -> RpcResult<Response<DestroyResponse>> {
        Ok(Response::new(DestroyResponse { success: true }))
    }
}

impl Default for ServerDaemonRuntime {
    fn default() -> Self {
        let info = ServerInfo::new(DEFAULT_HOST);
        let database = DeploymentDatabase::default();

        Self { info, database }
    }
}
