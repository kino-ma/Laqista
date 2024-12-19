use std::{collections::HashMap, time::Duration};

use laqista_core::{AppRpc, AppService, DeploymentInfo};
use prost_types::Timestamp;

use crate::{
    proto::{MonitorWindow, NodeAppStats, ResourceUtilization},
    utils::{subtract_window, IdMap},
    ServerInfo,
};

pub type StatsMap = IdMap<ServerStats>;
#[derive(Clone, Debug)]
pub struct ServerStats {
    pub server: ServerInfo,
    pub stats: Vec<MonitorWindow>,
}

impl ServerStats {
    pub fn new(server: ServerInfo) -> Self {
        let stats = Vec::new();
        Self { server, stats }
    }

    pub fn from_stats(server: ServerInfo, stats: Vec<MonitorWindow>) -> Self {
        Self { server, stats }
    }

    pub fn windows(&self) -> Windows {
        let inner = self.stats.iter();
        Windows { inner }
    }

    pub fn append(&mut self, mut window: Vec<MonitorWindow>) {
        self.stats.append(&mut window)
    }
}

pub struct Windows<'a> {
    inner: std::slice::Iter<'a, MonitorWindow>,
}

pub struct Window {
    pub start: Timestamp,
    pub end: Timestamp,
    pub nanos: i64,
    pub utilization: ResourceUtilization,
}

impl<'a> Iterator for Windows<'a> {
    type Item = Window;
    fn next(&mut self) -> Option<Self::Item> {
        let stats = self.inner.next()?;
        let window = stats.window.as_ref().expect("Start cannot be empty");

        let start = window.start.clone().expect("Start cannot be empty");
        let end = window.end.clone().expect("End cannot be empty");
        let nanos = subtract_window(&end, &start);
        let utilization = stats
            .utilization
            .clone()
            .expect("Utilization cannot be empty");

        Some(Window {
            start,
            end,
            nanos,
            utilization,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppsMap(HashMap<AppService, IdMap<AppLatency>>);

impl AppsMap {
    pub fn new(map: HashMap<AppService, IdMap<AppLatency>>) -> Self {
        Self(map)
    }

    pub fn get(&self, service: &AppService) -> Option<&IdMap<AppLatency>> {
        self.0.get(service)
    }

    pub fn insert(
        &mut self,
        server: &ServerInfo,
        deployment: &DeploymentInfo,
        rpc: &AppRpc,
        elapsed: Duration,
    ) {
        self.0
            .entry(rpc.clone().into())
            .and_modify(|e| {
                e.0.entry(server.id)
                    .and_modify(|latency| latency.insert(&rpc, elapsed))
                    .or_insert_with(|| {
                        let mut latency = AppLatency::new(deployment.clone());
                        latency.insert(rpc, elapsed);
                        latency
                    });
            })
            .or_insert_with(|| {
                let mut app_latency = AppLatency::new(deployment.clone());
                app_latency.insert(&rpc, elapsed);
                IdMap(HashMap::from([(server.id, app_latency)]))
            });
    }

    /// `AppsMap::filter_by_rpc()` filters latency history by rpc name.
    /// It returns a map from Node ID to RPC Latency.
    pub fn filter_by_rpc(&self, rpc: &AppRpc) -> IdMap<RpcLatency> {
        let app = match self.0.get(&rpc.to_owned().into()) {
            Some(a) => a,
            None => return IdMap::new(),
        };

        app.clone()
            .0
            .into_iter()
            .filter_map(|(id, lat)| lat.rpcs.get(rpc).map(|l| (id, l.clone())))
            .collect::<HashMap<_, _>>()
            .into()
    }

    /// { [app]: { [node_id]: latencies[] } } -> { [node_id]: { [rpc]: latencies } }
    pub fn into_node_stats(self) -> HashMap<String, NodeAppStats> {
        let mut out: HashMap<String, NodeAppStats> = HashMap::new();

        for (_svc, node_latencies) in self.0 {
            for (node_id, latency) in node_latencies.0 {
                for (rpc, latency) in latency.rpcs {
                    out.entry(node_id.to_string())
                        .and_modify(|per_rpc| {
                            per_rpc
                                .app_stats
                                .insert(rpc.to_string(), latency.average.as_millis() as _);
                        })
                        .or_insert(NodeAppStats {
                            app_stats: HashMap::from([(
                                rpc.to_string(),
                                latency.average.as_millis() as _,
                            )]),
                        });
                }
            }
        }

        out
    }
}

#[derive(Clone, Debug)]
pub struct AppLatency {
    pub info: DeploymentInfo,
    pub rpcs: HashMap<AppRpc, RpcLatency>,
}

impl AppLatency {
    pub fn new(info: DeploymentInfo) -> Self {
        let rpcs = HashMap::new();
        Self { info, rpcs }
    }

    pub fn insert(&mut self, rpc: &AppRpc, elapsed: Duration) {
        self.rpcs
            .entry(rpc.to_owned())
            .and_modify(|e| e.insert(elapsed))
            .or_insert_with(|| RpcLatency::with_first(elapsed));
    }

    pub fn lookup_service(&self, service: &AppService) -> HashMap<&AppRpc, &RpcLatency> {
        self.rpcs
            .iter()
            .filter(|(rpc, _)| service.contains(rpc))
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct RpcLatency {
    pub average: Duration,
    latencies: Vec<Duration>,
}

impl RpcLatency {
    pub fn with_first(elapsed: Duration) -> Self {
        Self {
            average: elapsed,
            latencies: vec![elapsed],
        }
    }
    pub fn insert(&mut self, elapsed: Duration) {
        let len = self.latencies.len() as _;
        self.average = (self.average * len + elapsed) / (len + 1);

        self.latencies.push(elapsed);
    }
}
