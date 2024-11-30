use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    utils::{is_hosts_equal, mul_as_percent},
    LocalitySpec, ServerInfo,
};

use super::{
    interface::DeploymentScheduler,
    stats::{AppsMap, ServerStats, StatsMap},
};

#[derive(Clone, Debug)]
pub struct MeanScheduler {}

const SCALEOUT_THREASHOLD: usize = 70;

impl DeploymentScheduler for MeanScheduler {
    fn schedule(
        &self,
        id: Uuid,
        name: &str,
        stats_map: &StatsMap,
        apps_map: &AppsMap,
        locality: &LocalitySpec,
    ) -> Option<ServerInfo> {
        let local_stats = self.filter_locality(stats_map.clone(), locality);

        if local_stats.is_empty() {
            println!("WARN: No servers matched locality specification: {locality:?}");
            return None;
        }

        let mut least_estimated = local_stats
            .iter()
            .next()
            .or_else(|| {
                println!("WARN: stats are empty");
                None
            })?
            .1;
        let mut least_estimated_latency = 0.;

        let server_latencies = apps_map.0.get(&id)?;

        for (id, stats) in local_stats.iter() {
            let utilized_rate = self.cpu_utilized_rate(stats);
            let free = 1. - utilized_rate;

            let latency = server_latencies.0.get(id)?.rpcs.get(name)?;

            // We consider greatest latency will become `free-resource-ratio * average-latency`
            let estimated_latency = free * (latency.average.as_millis() as f64);

            if estimated_latency < least_estimated_latency {
                least_estimated = &stats;
                least_estimated_latency = estimated_latency;
            }
        }

        Some(least_estimated.server.clone())
    }

    fn schedule_gpu(
        &self,
        id: Uuid,
        name: &str,
        stats_map: &StatsMap,
        apps_map: &AppsMap,
        locality: &LocalitySpec,
    ) -> Option<ServerInfo> {
        let local_stats = self.filter_locality(stats_map.clone(), locality);

        let mut least_estimated = local_stats
            .iter()
            .next()
            .or_else(|| {
                println!("WARN: stats are empty");
                None
            })?
            .1;
        let mut least_estimated_latency = 0.;

        let server_latencies = apps_map.0.get(&id)?;

        for (id, stats) in local_stats.iter() {
            let utilized_rate = self.gpu_utilized_rate(stats);
            let free = 1. - utilized_rate;

            let latency = server_latencies.0.get(id)?.rpcs.get(name)?;

            // We consider greatest latency will become `free-resource-ratio * average-latency`
            let estimated_latency = free * (latency.average.as_millis() as f64);

            if estimated_latency < least_estimated_latency {
                least_estimated = &stats;
                least_estimated_latency = estimated_latency;
            }
        }

        Some(least_estimated.server.clone())
    }

    fn least_utilized(&self, stats_map: &StatsMap) -> ServerInfo {
        let mut utils = stats_map
            .0
            .values()
            .map(|s| (s.server.clone(), self.cpu_utilized_rate(s)))
            .collect::<Vec<_>>();
        utils.sort_by_key(|t| (t.1 * 100.) as u64);
        utils[0].0.clone()
    }

    fn needs_scale_out(&self, _server: &ServerInfo, stats: &ServerStats) -> bool {
        let stat = match stats.stats.last() {
            Some(s) => s,
            None => return false,
        };

        return stat.utilization.as_ref().unwrap().cpu > SCALEOUT_THREASHOLD as _;
    }
}

impl MeanScheduler {
    fn filter_locality(
        &self,
        stats: StatsMap,
        locality: &LocalitySpec,
    ) -> HashMap<Uuid, ServerStats> {
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

    fn gpu_utilized_rate(&self, stats: &ServerStats) -> f64 {
        let total: f64 = stats.windows().map(|w| w.nanos as f64).sum();

        let utilized: f64 = stats
            .windows()
            .map(|w| mul_as_percent(w.nanos, w.utilization.gpu as _) as f64)
            .sum();

        utilized / total
    }

    fn cpu_utilized_rate(&self, stats: &ServerStats) -> f64 {
        let total: f64 = stats.windows().map(|w| w.nanos as f64).sum();

        let utilized: f64 = stats
            .windows()
            .map(|w| mul_as_percent(w.nanos, w.utilization.cpu as _) as f64)
            .sum();

        utilized / total
    }
}
