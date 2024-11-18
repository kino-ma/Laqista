use crate::{utils::mul_as_percent, ServerInfo};

use super::{
    interface::DeploymentScheduler,
    stats::{ServerStats, StatsMap},
};

#[derive(Clone, Debug)]
pub struct MeanScheduler {}

const SCALEOUT_THREASHOLD: usize = 70;

impl DeploymentScheduler for MeanScheduler {
    fn schedule(&self, stats_map: &StatsMap) -> Option<ServerInfo> {
        let mut least_utilized = stats_map
            .iter()
            .next()
            .or_else(|| {
                println!("WARN: stats are empty");
                None
            })?
            .1;
        let mut least_utilized_rate = 0.;

        for (_id, stats) in stats_map.iter() {
            let utilized_rate = self.cpu_utilized_rate(stats);

            if utilized_rate < least_utilized_rate {
                least_utilized = stats;
                least_utilized_rate = utilized_rate;
            }
        }

        Some(least_utilized.server.clone())
    }

    fn schedule_gpu(&self, stats_map: &StatsMap) -> Option<ServerInfo> {
        let mut least_utilized = stats_map
            .iter()
            .next()
            .or({
                println!("WARN: stats are empty");
                None
            })?
            .1;
        let mut least_utilized_rate = 0.;

        for (_id, stats) in stats_map.iter() {
            let utilized_rate = self.gpu_utilized_rate(stats);

            if utilized_rate < least_utilized_rate {
                least_utilized = stats;
                least_utilized_rate = utilized_rate;
            }
        }

        Some(least_utilized.server.clone())
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
