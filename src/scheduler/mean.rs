use core::f32;
use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use laqista_core::{AppRpc, AppService, DeploymentInfo};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    proto::MonitorWindow,
    scheduler::stats::RpcLatency,
    utils::{elapsed_from, is_hosts_equal, IdMap},
    LocalitySpec, QoSSpec, ServerInfo,
};

use super::{
    interface::{DeploymentScheduler, ScheduleResult},
    stats::{AppLatency, AppsMap, ServerStats, StatsMap},
};

#[derive(Clone, Debug, Default)]
pub struct MeanScheduler {
    history: HistoryLock,
}
type HistoryLock = Arc<Mutex<History>>;
type History = HashMap<AppRpc, (DateTime<Utc>, Uuid)>;

const SCALEOUT_THREASHOLD: usize = 70;

impl MeanScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_history(&self, rpc: AppRpc, server_id: Uuid) {
        match self.history.try_lock() {
            Ok(mut history) => {
                history.insert(rpc, (Utc::now(), server_id));
            }
            Err(_) => (),
        }
    }

    fn clone_history(&self) -> History {
        match self.history.try_lock() {
            Ok(history) => history.clone(),
            Err(_) => History::default(),
        }
    }

    /// `abstract_schedule()` defines abstract scheduling algorithm common for both cpu and gpu.
    /// It returns the least utilized node while satisfying QoS specifications.
    /// If no node can satisfy the requirement, it returns the node whose estimated latency is shortest.
    fn abstract_schedule<F>(
        &self,
        get_util: F,
        service: &AppService,
        app: &DeploymentInfo,
        stats_map: &StatsMap,
        apps_map: &AppsMap,
        qos: QoSSpec,
    ) -> Option<ScheduleResult>
    where
        F: Fn(&ServerStats) -> f64,
    {
        let history = self.clone_history();

        let required_accuracy = qos.accuracy.unwrap_or(f32::MIN);
        let required_latency = qos.latency.unwrap_or(u32::MAX);

        let available_rpcs = filter_rpcs_by_accuracy(app, required_accuracy);

        if available_rpcs.is_empty() {
            return None;
        }

        let local_stats = filter_locality(stats_map.clone(), &qos.locality);

        if local_stats.is_empty() {
            println!(
                "WARN: No servers matched locality specification: {:?}",
                qos.locality
            );
            return None;
        }

        let mut target = local_stats
            .iter()
            .next()
            .or_else(|| {
                println!("WARN: stats are empty");
                None
            })?
            .1;
        let mut target_rpc = AppRpc::new("", "", "");
        let mut target_latency = f64::MAX;
        let mut target_utilization = 100.0;
        let mut target_satisfies = false;

        let server_latencies = apps_map.get(service).or_else(|| None)?;

        for (server_id, stats) in local_stats.iter() {
            let utilized_rate = get_util(stats);
            let free = 1. - utilized_rate;
            let factor = 1. / if free > 0.0 { free } else { 0.01 };

            let latencies =
                get_rpc_latencies_or_default(server_id, server_latencies, &available_rpcs);

            for (rpc, latency) in latencies {
                let (last_call, last_target) = history
                    .get(&rpc)
                    .map(|t| t.clone())
                    .unwrap_or((Utc::now(), Uuid::max()));

                let time_left = if server_id == &last_target {
                    let elapsed = elapsed_from(&last_call);
                    latency.average.saturating_sub(elapsed)
                } else {
                    Duration::from_millis(0)
                };

                // We consider greatest latency will become `free-resource-ratio * average-latency`
                let estimated_latency =
                    factor * (latency.average.as_millis() as f64) + time_left.as_millis() as f64;

                let satisfies = estimated_latency <= required_latency as f64;
                let faster = estimated_latency <= target_latency;
                let less_utilized = utilized_rate < target_utilization;
                let both_free = utilized_rate <= 0.3 && target_utilization <= 0.3;

                // Select target if either:
                //   - No target has satisfied and faster
                //   - Satisfies the rquirements and less utilized
                if !target_satisfies && (satisfies || faster)
                    || satisfies && less_utilized
                    || (satisfies && both_free && faster)
                {
                    target = stats;
                    target_rpc = rpc.clone();
                    target_latency = estimated_latency;
                    target_utilization = utilized_rate;
                    target_satisfies = satisfies;
                }
            }
        }

        let needs_scale_out = is_over_utilized(&target) || !target_satisfies;

        if target_rpc.package == "" {
            None
        } else {
            self.insert_history(target_rpc.clone(), target.server.id);
            Some(ScheduleResult::new(
                target.server.clone(),
                target_rpc,
                needs_scale_out,
            ))
        }
    }
}

impl DeploymentScheduler for MeanScheduler {
    fn schedule(
        &self,
        rpc: &AppService,
        app: &DeploymentInfo,
        stats_map: &StatsMap,
        apps_map: &AppsMap,
        qos: QoSSpec,
    ) -> Option<ScheduleResult> {
        self.abstract_schedule(
            |s| cpu_utilized_rate(s),
            &rpc.to_owned().into(),
            app,
            stats_map,
            apps_map,
            qos,
        )
    }

    fn schedule_gpu(
        &self,
        rpc: &AppService,
        app: &DeploymentInfo,
        stats_map: &StatsMap,
        apps_map: &AppsMap,
        qos: QoSSpec,
    ) -> Option<ScheduleResult> {
        self.abstract_schedule(
            |s| gpu_utilized_rate(s),
            &rpc.to_owned().into(),
            app,
            stats_map,
            apps_map,
            qos,
        )
    }

    fn least_utilized(&self, stats_map: &StatsMap) -> ServerInfo {
        let mut utils = stats_map
            .0
            .values()
            .map(|s| (s.server.clone(), cpu_utilized_rate(s)))
            .collect::<Vec<_>>();
        utils.sort_by_key(|t| (t.1 * 100.) as u64);
        utils[0].0.clone()
    }
}

fn is_over_utilized(stats: &ServerStats) -> bool {
    let stat = match stats.stats.last() {
        Some(s) => s,
        None => return false,
    };

    return stat.utilization.as_ref().unwrap().cpu > SCALEOUT_THREASHOLD as _
        || stat.utilization.as_ref().unwrap().gpu > SCALEOUT_THREASHOLD as _;
}

fn gpu_utilized_rate(stats: &ServerStats) -> f64 {
    let Some(MonitorWindow {
        utilization: Some(utilization),
        ..
    }) = stats.stats.last()
    else {
        return 0.0;
    };

    utilization.gpu as f64 / 100.0
}

fn cpu_utilized_rate(stats: &ServerStats) -> f64 {
    let Some(MonitorWindow {
        utilization: Some(utilization),
        ..
    }) = stats.stats.last()
    else {
        return 0.0;
    };

    utilization.cpu as f64 / 100.0
}

fn filter_rpcs_by_accuracy(deployment: &DeploymentInfo, required_accuracy: f32) -> Vec<AppRpc> {
    deployment
        .accuracies
        .iter()
        .filter(|(_, acc)| **acc > required_accuracy)
        .map(|(rpc, _)| rpc.to_owned())
        .collect()
}

fn filter_locality(stats: StatsMap, locality: &LocalitySpec) -> HashMap<Uuid, ServerStats> {
    use LocalitySpec::*;

    if locality.is_some() {
        stats
            .0
            .into_iter()
            .filter(|(id, stats)| match locality {
                NodeId(spec_id) => id == spec_id,
                NodeHost(host) => is_hosts_equal(&host, &stats.server.addr),
                _ => {
                    unimplemented!(
                        "Scheduling with locality other than node id/host is not supproted"
                    )
                }
            })
            .collect::<HashMap<_, _>>()
    } else {
        stats.0
    }
}

fn get_rpc_latencies_or_default(
    server_id: &Uuid,
    latencies: &IdMap<AppLatency>,
    rpcs: &[AppRpc],
) -> Vec<(AppRpc, RpcLatency)> {
    assert!(rpcs.len() > 0);
    latencies
        .0
        .get(server_id)
        .map(|latencies| latencies.clone_by_rpcs(&rpcs))
        .unwrap_or_else(|| {
            println!("or else");
            rpcs.iter()
                .map(|rpc| {
                    (
                        (*rpc).clone(),
                        RpcLatency::with_first(Duration::from_millis(0)),
                    )
                })
                .collect()
        })
}

#[cfg(test)]
mod test {
    use std::time::{Duration, SystemTime};

    use crate::{
        proto::{MonitorWindow, ResourceUtilization, TimeWindow},
        scheduler::stats::AppLatency,
        utils::IdMap,
    };

    use super::*;

    #[test]
    fn return_fastest_unless_free() {
        let scheduler = MeanScheduler::new();
        let app = get_deployment();
        let service = AppService::new("laqista", "Scheduler");
        let stats = get_stats_map(None);
        let apps_map = get_apps_map(service.clone());
        let qos = QoSSpec {
            latency: None,
            accuracy: None,
            locality: LocalitySpec::None,
        };

        let Some(res) = scheduler.schedule(&service, &app, &stats, &apps_map, qos) else {
            panic!("failed to schedule")
        };

        assert_eq!(res.server.id, Uuid::from_u128(3));
        assert_eq!(res.rpc, service.rpc("fifty"));
    }

    #[test]
    fn satisfy_accuracy_requirement() {
        let scheduler = MeanScheduler::new();
        let app = get_deployment();
        let service = AppService::new("laqista", "Scheduler");
        let stats = get_stats_map(None);
        let apps_map = get_apps_map(service.clone());
        let qos = QoSSpec {
            latency: None,
            accuracy: Some(65.),
            locality: LocalitySpec::None,
        };

        let Some(res) = scheduler.schedule(&service, &app, &stats, &apps_map, qos) else {
            panic!("failed to schedule")
        };

        assert_eq!(res.server.id, Uuid::from_u128(3));
        assert_eq!(res.rpc, service.rpc("seventy"));
    }

    /// Estimated times:
    ///
    /// - A: 1.0 / (1.0-0.4) * 50  ->  66.6
    /// - B: 1.0 / (1.0-0.1) * 100 -> 111.1
    /// - C: 1.0 / (1.0-0.7) * 40  -> 133.3
    ///
    /// Because no latency QoS specified, most free one, B should be chosen.
    #[test]
    fn return_most_free() {
        let scheduler = MeanScheduler::new();
        let app = get_deployment();
        let service = AppService::new("laqista", "Scheduler");
        let stats = get_stats_map(Some((40., 10., 70.)));
        let apps_map = get_apps_map(service.clone());
        let qos = QoSSpec {
            latency: None,
            accuracy: None,
            locality: LocalitySpec::None,
        };

        let Some(res) = scheduler.schedule(&service, &app, &stats, &apps_map, qos) else {
            panic!("failed to schedule")
        };

        assert_eq!(res.server.id, Uuid::from_u128(2));
        assert_eq!(res.rpc, service.rpc("fifty"));
    }

    /// Estimated times:
    ///
    /// - A: 1.0 / (1.0-0.4) * 60  ->  80.0
    /// - B: 1.0 / (1.0-0.1) * 120 -> 133.3
    /// - C: 1.0 / (1.0-0.7) * 48  -> 160.0
    ///
    /// With latency req. 100 and accuarcy req. 55%, only A and "sixty" can be chosen.
    #[test]
    fn satisfy_latency_and_accuracy() {
        let scheduler = MeanScheduler::new();
        let app = get_deployment();
        let service = AppService::new("laqista", "Scheduler");
        let stats = get_stats_map(Some((40., 10., 70.)));
        let apps_map = get_apps_map(service.clone());
        let qos = QoSSpec {
            latency: Some(100),
            accuracy: Some(55.),
            locality: LocalitySpec::None,
        };

        let Some(res) = scheduler.schedule(&service, &app, &stats, &apps_map, qos) else {
            panic!("failed to schedule")
        };

        // Should return fastest rpc & host
        assert_eq!(res.server.id, Uuid::from_u128(1));
        assert_eq!(res.rpc, service.rpc("sixty"));
    }

    fn get_deployment() -> DeploymentInfo {
        let service = AppService::new("laqista", "Scheduler");
        let services = HashMap::from([(
            service.clone(),
            vec![
                service.rpc("fifty"),
                service.rpc("sixty"),
                service.rpc("seventy"),
            ],
        )]);

        DeploymentInfo {
            id: Uuid::default(),
            name: "laqista".to_owned(),
            source: "https://github.com/kino-ma/Laqista/releases/download/v0.1.0/face_v0.1.0.tgz"
                .to_owned(),
            services,
            accuracies: HashMap::from([
                (service.rpc("fifty"), 50.),
                (service.rpc("sixty"), 60.),
                (service.rpc("seventy"), 70.),
            ]),
        }
    }

    fn get_server(i: u128) -> ServerInfo {
        ServerInfo {
            id: Uuid::from_u128(i),
            addr: format!("http://127.0.0.{i}:50051"),
        }
    }

    fn get_server_stats(i: u128, util: f32) -> ServerStats {
        let mut s = ServerStats::new(get_server(i));
        let u = util as _;
        let window = MonitorWindow {
            window: Some(TimeWindow {
                start: Some(SystemTime::now().into()),
                end: Some(
                    SystemTime::now()
                        .checked_add(Duration::from_secs(1))
                        .unwrap()
                        .into(),
                ),
            }),
            utilization: Some(ResourceUtilization {
                cpu: u,
                ram_total: u,
                ram_used: u,
                gpu: u,
                vram_total: u,
                vram_used: u,
            }),
        };
        s.append(vec![window]);

        s
    }

    fn get_stats_map(utils: Option<(f32, f32, f32)>) -> StatsMap {
        let utils = utils.unwrap_or_default();
        let map = HashMap::from([
            (Uuid::from_u128(1), get_server_stats(1, utils.0)),
            (Uuid::from_u128(2), get_server_stats(2, utils.1)),
            (Uuid::from_u128(3), get_server_stats(3, utils.2)),
        ]);

        IdMap(map)
    }

    fn get_app_latency(latencies: &[(&str, u32)]) -> AppLatency {
        let mut l = AppLatency::new(get_deployment());

        for (name, ms) in latencies {
            let rpc = AppRpc::new("laqista", "Scheduler", name);
            let elapsed = Duration::from_millis(*ms as _);
            l.insert(&rpc, elapsed);
        }

        l
    }

    fn get_apps_map(service: AppService) -> AppsMap {
        let latency_map = HashMap::from([
            (
                Uuid::from_u128(1),
                get_app_latency(&[("fifty", 50), ("sixty", 60), ("seventy", 70)]),
            ),
            (
                Uuid::from_u128(2),
                get_app_latency(&[("fifty", 100), ("sixty", 120), ("seventy", 140)]),
            ),
            (
                Uuid::from_u128(3),
                get_app_latency(&[("fifty", 40), ("sixty", 48), ("seventy", 56)]),
            ),
        ]);
        let map = HashMap::from([(service, IdMap(latency_map))]);

        AppsMap::new(map)
    }
}
