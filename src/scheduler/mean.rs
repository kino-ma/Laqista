use uuid::Uuid;

use crate::{utils::mul_as_percent, ServerInfo};

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
    ) -> Option<ServerInfo> {
        let mut least_estimated = stats_map
            .iter()
            .next()
            .or_else(|| {
                println!("WARN: stats are empty");
                None
            })?
            .1;
        let mut least_estimated_latency = 0.;

        let server_latencies = apps_map.0.get(&id)?;

        for (id, stats) in stats_map.iter() {
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
    ) -> Option<ServerInfo> {
        let mut least_estimated = stats_map
            .iter()
            .next()
            .or_else(|| {
                println!("WARN: stats are empty");
                None
            })?
            .1;
        let mut least_estimated_latency = 0.;

        let server_latencies = apps_map.0.get(&id)?;

        for (id, stats) in stats_map.iter() {
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
