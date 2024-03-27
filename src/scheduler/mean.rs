use crate::{utils::mul_as_percent, ServerInfo};

use super::{
    interface::DeploymentScheduler,
    stats::{ServerStats, StatsMap},
};

pub struct MeanGpuScheduler {}

impl DeploymentScheduler for MeanGpuScheduler {
    fn schedule(&self, stats_map: &StatsMap) -> Option<&ServerInfo> {
        let mut least_utilized = stats_map.iter().next()?.1;
        let mut least_utilized_rate = 0.;

        for (id, stats) in stats_map.iter() {
            let utilized_rate = self.utilized_rate(stats);

            if utilized_rate < least_utilized_rate {
                least_utilized = stats;
                least_utilized_rate = utilized_rate;
            }
        }

        Some(&least_utilized.server)
    }
}

impl MeanGpuScheduler {
    fn utilized_rate(&self, stats: &ServerStats) -> f64 {
        let total: f64 = stats.windows().map(|w| w.nanos as f64).sum();

        let utilized: f64 = stats
            .windows()
            .map(|w| mul_as_percent(w.nanos, w.utilization.gpu as _) as f64)
            .sum();

        utilized / total
    }
}
