use crate::ServerInfo;

use super::stats::StatsMap;

pub trait DeploymentScheduler: SchedulerClone + std::fmt::Debug + Send + Sync {
    fn schedule(&self, stats: &StatsMap) -> Option<ServerInfo>;
    fn schedule_gpu(&self, stats: &StatsMap) -> Option<ServerInfo>;
}

pub trait SchedulerClone {
    fn clone_box(&self) -> Box<dyn DeploymentScheduler>;
}

impl<T> SchedulerClone for T
where
    T: 'static + DeploymentScheduler + Clone,
{
    fn clone_box(&self) -> Box<dyn DeploymentScheduler> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn DeploymentScheduler> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
