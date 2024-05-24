pub mod radeon;
pub use radeon::*;
use tokio::{sync::mpsc::Sender, task::JoinHandle};

use super::SendMetrics;

mod parse;

pub struct MetricsMonitor {}

impl MetricsMonitor {
    pub fn new() -> Self {
        MetricsMonitor {}
    }
}

impl SendMetrics for MetricsMonitor {
    fn spawn(&self, tx: Sender<crate::proto::MonitorWindow>) -> JoinHandle<()> {
        tokio::spawn(async move {
            use HostSystem::*;

            match HostSystem::determine() {
                Radeon => {
                    let monitor = RadeonMonitor::new();
                    monitor.run(tx).await;
                }
                _ => unimplemented!("Unsupported platform"),
            }
        })
    }
}

#[allow(unused)]
enum HostSystem {
    Nvidia,
    Radeon,
    Unknown,
}

impl HostSystem {
    pub fn determine() -> Self {
        //TODO: Deteremine underlying system
        Self::Radeon
    }
}
