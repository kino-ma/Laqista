#![feature(test)]

use std::result::Result as StdResult;

use laqista_core::DeploymentInfo;
use proto::{AppInstanceLocations, Deployment, Group, Locality, Server, ServerState};
use server::DaemonState;
use tonic::Status;
use utils::{get_mac, IdMap};
use uuid::Uuid;

pub mod cmd;
pub mod deployment;
pub mod error;
pub mod monitor;
pub mod proxy;
pub mod report;
pub mod scheduler;
pub mod server;
mod utils;

pub mod proto {
    tonic::include_proto!("laqista");
}

pub use error::{Error, Result};
pub type RpcResult<T> = StdResult<T, Status>;

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

#[derive(Clone, Debug)]
pub struct AppInstancesInfo {
    deployment: DeploymentInfo,
    servers: Vec<ServerInfo>,
}
pub type AppInstanceMap = IdMap<AppInstancesInfo>;

impl ServerInfo {
    pub fn new(host: &str) -> Self {
        let id = Self::gen_id().unwrap();

        Self::with_id(host, id)
    }

    pub fn with_id(host: &str, id: Uuid) -> Self {
        let addr = format!("http://{}", host);
        Self { id, addr }
    }

    pub fn with_id_str(id: &str, host: &str) -> Result<Self> {
        let id = Uuid::try_parse(&id)?;
        Ok(Self::with_id(host, id))
    }

    fn gen_id() -> Result<Uuid> {
        let mac = get_mac()?;
        Ok(Uuid::now_v6(&mac.bytes()))
    }
}

impl GroupInfo {
    pub fn new(scheduler_info: &ServerInfo) -> Self {
        let scheduler_info = scheduler_info.clone();
        Self {
            number: 0,
            scheduler_info,
        }
    }

    pub fn with_number(scheduler_info: &ServerInfo, number: u32) -> Self {
        let scheduler_info = scheduler_info.clone();
        Self {
            number,
            scheduler_info,
        }
    }
}

impl Into<Server> for ServerInfo {
    fn into(self) -> Server {
        let Self { id, addr } = self.clone();
        let id = id.into();

        Server { id, addr }
    }
}

impl TryFrom<Server> for ServerInfo {
    type Error = Error;
    fn try_from(server: Server) -> Result<Self> {
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
    type Error = Error;
    fn try_from(group: Group) -> Result<Self> {
        let Group { number, scheduler } = group;

        let scheduler_info = match scheduler {
            Some(s) => s.try_into()?,
            None => return Err("No scheduler".to_owned())?,
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
            Self::Cloud(_) => Cloud,
            Self::Fog(_) => Fog,
            Self::Dew(_) => Dew,
            Self::Joining(_) => Starting,
            Self::Authoritative(_) => Authoritative,
            Self::Failed => Failed,
        }
    }
}

impl TryFrom<Deployment> for DeploymentInfo {
    type Error = Error;
    fn try_from(deployment: Deployment) -> Result<Self> {
        let Deployment {
            name,
            source,
            id,
            accuracies_percent,
        } = deployment;
        let id = Uuid::parse_str(&id)?;
        Ok(Self {
            name,
            source,
            id,
            accuracies: accuracies_percent,
        })
    }
}

impl Into<Deployment> for DeploymentInfo {
    fn into(self) -> Deployment {
        let Self {
            name,
            source,
            id,
            accuracies,
        } = self;
        let id = id.to_string();
        Deployment {
            name,
            source,
            id,
            accuracies_percent: accuracies,
        }
    }
}

impl Into<AppInstanceLocations> for AppInstancesInfo {
    fn into(self) -> AppInstanceLocations {
        let deployment = Some(self.deployment.into());
        let locations = self.servers.iter().map(|s| s.clone().into()).collect();

        AppInstanceLocations {
            deployment,
            locations,
        }
    }
}

impl TryFrom<AppInstanceLocations> for AppInstancesInfo {
    type Error = Error;
    fn try_from(locations: AppInstanceLocations) -> Result<Self> {
        let deployment = locations
            .deployment
            .ok_or("Deployment cannot be empty".to_string())?
            .try_into()?;

        let servers = locations
            .locations
            .into_iter()
            .map(ServerInfo::try_from)
            .collect::<Result<_>>()?;

        Ok(Self {
            deployment,
            servers,
        })
    }
}

#[derive(Clone, Debug)]
pub enum LocalitySpec {
    NodeId(Uuid),
    NodeHost(String),
    PublicKey(String),
    None,
}

impl LocalitySpec {
    pub fn id(self) -> Option<Uuid> {
        match self {
            Self::NodeId(id) => Some(id),
            _ => None,
        }
    }

    pub fn host(self) -> Option<String> {
        match self {
            Self::NodeHost(host) => Some(host),
            _ => None,
        }
    }

    pub fn pubkey(self) -> Option<String> {
        match self {
            Self::PublicKey(key) => Some(key),
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            Self::None => true,
            _ => false,
        }
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
}

impl TryFrom<Locality> for LocalitySpec {
    type Error = Error;
    fn try_from(locality: Locality) -> StdResult<Self, Self::Error> {
        use proto::locality::Specification;

        let spec = locality.specification.ok_or(Error::NoneError)?;
        match spec {
            Specification::Id(id) => Ok(Uuid::try_parse(&id).map(Self::NodeId)?),
            Specification::Host(host) => Ok(Self::NodeHost(host)),
            Specification::Pubkey(key) => Ok(Self::PublicKey(key)),
            Specification::None(_) => Ok(Self::None),
        }
    }
}

impl TryFrom<Option<Locality>> for LocalitySpec {
    type Error = Error;
    fn try_from(locality: Option<Locality>) -> StdResult<Self, Self::Error> {
        use proto::locality::Specification;

        let spec = if let Some(l) = locality {
            l.specification.ok_or(Error::NoneError)?
        } else {
            return Ok(Self::None);
        };

        match spec {
            Specification::Id(id) => Ok(Uuid::try_parse(&id).map(Self::NodeId)?),
            Specification::Host(host) => Ok(Self::NodeHost(host)),
            Specification::Pubkey(key) => Ok(Self::PublicKey(key)),
            Specification::None(_) => Ok(Self::None),
        }
    }
}
