#![feature(async_closure)]
#![feature(async_fn_traits)]

pub mod client;
pub mod proto;
pub mod server;
pub mod session;
pub mod tensor;
pub mod wasm;

use std::{collections::HashMap, str::FromStr};

use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AppService {
    pub package: String,
    pub service: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AppRpc {
    pub package: String,
    pub service: String,
    pub rpc: String,
}

#[derive(Clone, Debug)]
pub struct DeploymentInfo {
    pub id: Uuid,
    pub name: String,
    pub source: String,
    pub services: HashMap<AppService, Vec<AppRpc>>,
    pub accuracies: HashMap<AppRpc, f32>,
}

impl DeploymentInfo {
    pub fn new(
        name: String,
        source: String,
        services: HashMap<AppService, Vec<AppRpc>>,
        accuracies: HashMap<AppRpc, f32>,
    ) -> Self {
        let id = Uuid::new_v4();

        Self {
            name,
            source,
            id,
            services,
            accuracies,
        }
    }

    pub fn from_rpcs<S: AsRef<str>>(
        name: String,
        source: String,
        rpcs: &[S],
        accuracies: HashMap<AppRpc, f32>,
    ) -> Option<Self> {
        let mut services = HashMap::new();

        for rpc_name in rpcs {
            let rpc = AppRpc::from_str(rpc_name.as_ref()).ok()?;
            let service = rpc.clone().into();
            services
                .entry(service)
                .and_modify(|v: &mut Vec<AppRpc>| v.push(rpc.clone()))
                .or_insert(vec![rpc]);
        }

        Some(Self::new(name, source, services, accuracies))
    }
}

impl AppService {
    pub fn new(package: &str, service: &str) -> Self {
        Self {
            package: package.to_owned(),
            service: service.to_owned(),
        }
    }

    pub fn contains(&self, rpc: &AppRpc) -> bool {
        self.package == rpc.package && self.service == rpc.service
    }

    pub fn rpc(&self, name: &str) -> AppRpc {
        AppRpc::new(&self.package, &self.service, name)
    }
}

impl FromStr for AppService {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // /package.Service/Rpc => package.Service
        let s = s.split("/").next().ok_or(())?;

        let mut splitted = s.split(".");
        let package = splitted.next().ok_or(())?;
        let service = splitted.next().ok_or(())?;

        Ok(Self::new(package, service))
    }
}

impl AppRpc {
    pub fn new(package: &str, service: &str, rpc: &str) -> Self {
        Self {
            package: package.to_owned(),
            service: service.to_owned(),
            rpc: rpc.to_owned(),
        }
    }
}

impl Into<AppService> for AppRpc {
    fn into(self) -> AppService {
        AppService::new(&self.package, &self.service)
    }
}

impl FromStr for AppRpc {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut paths = s.split("/").skip(1);

        let pkg_svc = paths.next().ok_or(())?;
        let rpc = paths.next().ok_or(())?;

        let mut iter = pkg_svc.split(".");
        let pkg = iter.next().ok_or(())?;
        let svc = iter.next().ok_or(())?;

        Ok(Self::new(pkg, svc, rpc))
    }
}

impl ToString for AppRpc {
    fn to_string(&self) -> String {
        format!("/{}.{}/{}", self.package, self.service, self.rpc)
    }
}

pub fn try_collect_accuracies(
    accuracies_percent: HashMap<String, f32>,
) -> Option<HashMap<AppRpc, f32>> {
    let results: Vec<Result<_, _>> = accuracies_percent
        .into_iter()
        .map(|(path, acc)| AppRpc::from_str(&path).map(|r| (r, acc)))
        .collect();

    results.into_iter().collect::<Result<_, _>>().ok()
}
