use crate::ServerInfo;

use super::stats::StatsMap;

pub trait DeploymentScheduler: Send + Sync {
    fn schedule(&self, stats: &StatsMap) -> Option<&ServerInfo>;
}
