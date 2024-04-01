#![feature(test)]

use std::{error::Error, net::SocketAddr};

use mac_address::MacAddressError;
use proto::{AppInstanceLocations, Deployment, Group, Server, ServerState};
use server::DaemonState;
use url::Url;
use utils::{get_mac, IdMap};
use uuid::Uuid;

pub mod cmd;
pub mod monitor;
pub mod proxy;
pub mod report;
pub mod scheduler;
pub mod server;
mod utils;

#[cfg(test)]
mod bench;

// sample application
mod app;
// mod bench;

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

#[derive(Clone, Debug)]
pub struct DeploymentInfo {
    id: Uuid,
    source: String,
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

        Self::with_id(host, &id)
    }

    pub fn with_id(host: &str, id: &Uuid) -> Self {
        let id = id.clone();
        let addr = format!("http://{}", host);
        Self { id, addr }
    }

    fn gen_id() -> Result<Uuid, MacAddressError> {
        let mac = get_mac()?;
        Ok(Uuid::now_v6(&mac.bytes()))
    }

    pub fn as_socket(&self) -> Result<SocketAddr, Box<dyn Error>> {
        let parsed = Url::parse(&self.addr)?;
        let mut hosts = parsed.socket_addrs(|| None)?;
        return Ok(hosts.pop().ok_or("could not find any hosts".to_string())?);
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
            Self::Starting => Starting,
            Self::Running(_) => Running,
            Self::Uninitialized => Uninitialized,
            Self::Joining(_) => Starting,
            Self::Authoritative(_) => Authoritative,
            Self::Failed => Failed,
        }
    }
}

impl DeploymentInfo {
    pub fn new(source: String) -> Self {
        let id = Uuid::new_v4();
        Self { source, id }
    }
}

impl TryFrom<Deployment> for DeploymentInfo {
    type Error = String;
    fn try_from(deployment: Deployment) -> Result<Self, Self::Error> {
        let Deployment { source, id } = deployment;
        let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
        Ok(Self { source, id })
    }
}

impl Into<Deployment> for DeploymentInfo {
    fn into(self) -> Deployment {
        let Self { source, id } = self;
        let id = id.to_string();
        Deployment { source, id }
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
    type Error = String;
    fn try_from(locations: AppInstanceLocations) -> Result<Self, Self::Error> {
        let deployment = locations
            .deployment
            .ok_or("Deployment cannot be empty".to_string())?
            .try_into()?;

        let servers = locations
            .locations
            .into_iter()
            .map(ServerInfo::try_from)
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            deployment,
            servers,
        })
    }
}
