use proto::{GetInfoResponse, Group, Server, ServerState};
use server::DaemonState;
use uuid::Uuid;

pub mod server;

mod utils;

pub mod proto {
    tonic::include_proto!("mless");
}

#[derive(Clone, Debug)]
pub struct ServerInfo {
    id: Uuid,
    addr: String,
}

#[derive(Clone, Debug)]
pub struct GroupInfo {
    number: u32,
    scheduler_info: ServerInfo,
}

impl Into<Server> for ServerInfo {
    fn into(self) -> Server {
        let Self { id, addr } = self.clone();
        let id = id.into();

        Server { id, addr }
    }
}

impl TryFrom<Server> for ServerInfo {
    type Error = uuid::Error;
    fn try_from(server: Server) -> Result<Self, Self::Error> {
        let Server { id, addr } = server.clone();
        let id = Uuid::parse_str(&id)?;

        Ok(Self { id, addr })
    }
}

impl Into<Group> for GroupInfo {
    fn into(self) -> Group {
        let Self {
            number,
            scheduler_info,
        } = self.clone();
        let scheduler = Some(scheduler_info.into());

        Group { number, scheduler }
    }
}

impl TryFrom<Group> for GroupInfo {
    type Error = String;
    fn try_from(group: Group) -> Result<Self, Self::Error> {
        let Group { number, scheduler } = group;

        let scheduler_info = match scheduler {
            Some(s) => s.try_into().map_err(|e: uuid::Error| e.to_string())?,
            None => return Err("No scheduler".into()),
        };

        Ok(Self {
            number,
            scheduler_info,
        })
    }
}

impl Into<ServerState> for DaemonState {
    fn into(self) -> ServerState {
        use ServerState::*;

        match self {
            Self::Uninitialized => Uninitialized,
            Self::Running(_) => Running,
            Self::Failed => Failed,
        }
    }
}

pub fn get_daemon_state(info: &GetInfoResponse) -> Result<DaemonState, String> {
    use ServerState::*;

    let state = ServerState::try_from(info.state).map_err(|e| e.to_string());

    let group = match info.group.clone() {
        Some(group) => GroupInfo::try_from(group).map_err(|e| e.to_string()),
        None => Err("No group info".to_string()),
    }?;

    match state {
        Ok(Uninitialized) => Ok(DaemonState::Uninitialized),
        Ok(Running) => Ok(DaemonState::Running(group)),
        Ok(Failed) => Ok(DaemonState::Failed),
        Err(e) => Err(e.to_string()),
    }
}
