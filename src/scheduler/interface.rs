use laqista_core::{AppRpc, AppService, DeploymentInfo};

use crate::{QoSSpec, ServerInfo};

use super::stats::{AppsMap, StatsMap};

pub trait DeploymentScheduler: SchedulerClone + std::fmt::Debug + Send + Sync {
    fn schedule(
        &self,
        rpc: &AppService,
        app: &DeploymentInfo,
        stats: &StatsMap,
        apps_map: &AppsMap,
        qos: QoSSpec,
    ) -> Option<ScheduleResult>;

    fn schedule_gpu(
        &self,
        rpc: &AppService,
        app: &DeploymentInfo,
        stats: &StatsMap,
        apps_map: &AppsMap,
        qos: QoSSpec,
    ) -> Option<ScheduleResult>;

    fn least_utilized(&self, stats: &StatsMap) -> ServerInfo;
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

#[derive(Clone, Debug)]
pub struct ScheduleResult {
    pub server: ServerInfo,
    pub rpc: AppRpc,
    pub needs_scale_out: bool,
}

impl ScheduleResult {
    pub fn new(server: ServerInfo, rpc: AppRpc, needs_scale_out: bool) -> Self {
        ScheduleResult {
            server,
            rpc,
            needs_scale_out,
        }
    }
}
