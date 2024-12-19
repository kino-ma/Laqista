use std::sync::{Arc, Mutex};

use super::interface::{DeploymentScheduler, ScheduleResult};

#[derive(Clone, Debug)]
pub struct RoundRobbinScheduler {
    last: Arc<Mutex<usize>>,
}

impl RoundRobbinScheduler {
    pub fn new() -> Self {
        let last = Arc::new(Mutex::new(0));
        Self { last }
    }
}

impl DeploymentScheduler for RoundRobbinScheduler {
    fn schedule(
        &self,
        service: &laqista_core::AppService,
        app: &laqista_core::DeploymentInfo,
        stats: &super::stats::StatsMap,
        _apps_map: &super::stats::AppsMap,
        _qos: crate::QoSSpec,
    ) -> Option<super::interface::ScheduleResult> {
        let mut ptr = self.last.lock().unwrap();
        *ptr += 1;
        if *ptr >= stats.0.len() {
            *ptr = 0;
        }

        let stat = stats.0.iter().nth(*ptr)?;
        let rpc = app.services.get(service).unwrap()[0].clone();
        Some(ScheduleResult {
            server: stat.1.server.clone(),
            rpc,
            needs_scale_out: true,
        })
    }

    fn schedule_gpu(
        &self,
        service: &laqista_core::AppService,
        app: &laqista_core::DeploymentInfo,
        stats: &super::stats::StatsMap,
        _apps_map: &super::stats::AppsMap,
        _qos: crate::QoSSpec,
    ) -> Option<super::interface::ScheduleResult> {
        let mut ptr = self.last.lock().unwrap();
        *ptr += 1;
        if *ptr >= stats.0.len() {
            *ptr = 0;
        }

        let stat = stats.0.iter().nth(*ptr)?;
        let rpc = app.services.get(service).unwrap()[0].clone();
        Some(ScheduleResult {
            server: stat.1.server.clone(),
            rpc,
            needs_scale_out: true,
        })
    }

    fn least_utilized(&self, stats: &super::stats::StatsMap) -> crate::ServerInfo {
        let mut ptr = self.last.lock().unwrap();
        *ptr += 1;
        if *ptr >= stats.0.len() {
            *ptr = 0;
        }

        let stat = stats.0.iter().nth(*ptr).unwrap();

        stat.1.server.clone()
    }
}
