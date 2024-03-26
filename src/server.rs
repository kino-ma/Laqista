use std::error::Error;

use tonic::{transport::Server as TransportServer, Request, Response, Status};
use uuid::Uuid;

use crate::proto::server_daemon_server::{ServerDaemon, ServerDaemonServer};
use crate::proto::{
    DestroyRequest, DestroyResponse, GetInfoRequest, GetInfoResponse, MonitorRequest,
    MonitorResponse, PingResponse, ServerState, SpawnRequest, SpawnResponse,
};
use crate::utils::get_mac;
use crate::{GroupInfo, ServerInfo};

#[derive(Clone, Debug)]
pub struct ServerDaemonRuntime {
    info: ServerInfo,
    state: DaemonState,
}

#[derive(Clone, Debug)]
pub enum DaemonState {
    Uninitialized,
    Running(GroupInfo),
    Failed,
}

impl ServerDaemonRuntime {
    pub fn new(id: Uuid, addr: &str, state: DaemonState) -> Self {
        let info = ServerInfo {
            id,
            addr: addr.to_owned(),
        };

        Self { info, state }
    }

    pub fn create() -> Result<Self, Box<dyn Error>> {
        let mac = get_mac()?;
        let id = Uuid::now_v6(&mac.bytes());

        let state = DaemonState::Uninitialized;

        Ok(Self::new(id, "127.0.0.1:50051", state))
    }

    pub async fn start(self) -> Result<(), Box<dyn Error>> {
        let addr = self.info.addr.parse()?;

        println!("starting server... ({})", self.info.id);

        TransportServer::builder()
            .add_service(ServerDaemonServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl ServerDaemon for ServerDaemonRuntime {
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> Result<Response<GetInfoResponse>, Status> {
        println!("GetInfo called!");

        let server = Some(self.info.clone().into());

        use DaemonState::*;
        let group = match &self.state {
            Uninitialized => None,
            Running(group) => Some(group.clone().into()),
            Failed => None,
        };

        let state: ServerState = self.state.clone().into();
        let state = state.into();

        let resposne = GetInfoResponse {
            server,
            group,
            state,
        };

        Ok(Response::new(resposne))
    }

    async fn ping(&self, _request: Request<()>) -> Result<Response<PingResponse>, Status> {
        println!("got ping!");

        let resposne = PingResponse { success: true };

        Ok(Response::new(resposne))
    }

    async fn monitor(
        &self,
        _request: Request<MonitorRequest>,
    ) -> Result<Response<MonitorResponse>, Status> {
        Ok(Response::new(MonitorResponse { windows: vec![] }))
    }
    async fn spawn(
        &self,
        _request: Request<SpawnRequest>,
    ) -> Result<Response<SpawnResponse>, Status> {
        Ok(Response::new(SpawnResponse {
            success: true,
            deployment: None,
            server: None,
        }))
    }
    async fn destroy(
        &self,
        _request: Request<DestroyRequest>,
    ) -> Result<Response<DestroyResponse>, Status> {
        Ok(Response::new(DestroyResponse { success: true }))
    }
}

impl Default for ServerDaemonRuntime {
    fn default() -> Self {
        Self::create().expect("failed to create default daemon")
    }
}
