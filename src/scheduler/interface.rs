use crate::ServerInfo;

use super::stats::StatsMap;

pub trait DeploymentScheduler {
    fn schedule(&self, stats: &StatsMap) -> Option<&ServerInfo>;
}
