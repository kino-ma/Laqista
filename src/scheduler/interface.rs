use uuid::Uuid;

use crate::{QoSSpec, ServerInfo};

use super::stats::{AppsMap, ServerStats, StatsMap};

pub trait DeploymentScheduler: SchedulerClone + std::fmt::Debug + Send + Sync {
    fn schedule(
        &self,
        id: Uuid,
        name: &str,
        stats: &StatsMap,
        apps_map: &AppsMap,
        qos: QoSSpec,
    ) -> Option<ServerInfo>;

    fn schedule_gpu(
        &self,
        id: Uuid,
        name: &str,
        stats: &StatsMap,
        apps_map: &AppsMap,
        qos: QoSSpec,
    ) -> Option<ServerInfo>;

    fn least_utilized(&self, stats: &StatsMap) -> ServerInfo;
    fn needs_scale_out(&self, server: &ServerInfo, stats: &ServerStats) -> bool;
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
