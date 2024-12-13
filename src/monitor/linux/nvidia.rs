use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use nvml_wrapper::{
    error::NvmlError, struct_wrappers::device::Utilization as GpuUtilization, Device, Nvml,
};
use systemstat::{CPULoad, Platform, System};
use tokio::{sync::mpsc, time};

use crate::{
    proto::{MonitorWindow, ResourceUtilization, TimeWindow},
    utils::datetime_to_prost,
};

pub struct NvidiaMonitor {
    nvml: Nvml,
    sys: System,
}

impl NvidiaMonitor {
    pub fn new() -> Result<Self, NvmlError> {
        let nvml = Nvml::init()?;
        let sys = System::new();
        Ok(Self { nvml, sys })
    }

    pub async fn run(&self, tx: mpsc::Sender<MonitorWindow>) -> ! {
        let count = self
            .nvml
            .device_count()
            .expect("failed to get device count");

        let devices: Vec<Device> = (0..count)
            .filter_map(|i| {
                self.nvml
                    .device_by_index(i)
                    .map_err(|e| println!("WARN: failed to get NVIDIA device #{i}: {e:?}"))
                    .ok()
            })
            .collect();

        loop {
            let timestamp = Utc::now();

            let cpu_measurement = match self.sys.cpu_load_aggregate() {
                Ok(m) => m,
                Err(e) => {
                    println!("WARN: failed to start measuring CPU utilization: {e:?}");
                    continue;
                }
            };

            time::sleep(Duration::from_secs(1)).await;

            let cpu_load = match cpu_measurement.done() {
                Ok(load) => load,
                Err(e) => {
                    println!("WARN: failed to aggregate CPU utilization: {e:?}");
                    continue;
                }
            };

            let utils = devices
                .iter()
                .filter_map(|d| {
                    d.utilization_rates()
                        .map_err(|e| println!("WARN: failed to get utilization rate: {e:?}"))
                        .ok()
                })
                .collect::<Vec<_>>();

            let metrics = NvidiaMetrics {
                timestamp,
                utils,
                cpu_load,
            };

            tx.send(metrics.into())
                .await
                .unwrap_or_else(|e| println!("WARN: falied to send metrics: {e:?}"));
        }
    }
}

#[derive(Clone, Debug)]
pub struct NvidiaMetrics {
    pub timestamp: DateTime<Utc>,
    pub utils: Vec<GpuUtilization>,
    pub cpu_load: CPULoad,
}

impl NvidiaMetrics {
    pub fn total_utilization_rate(&self) -> f64 {
        let len = self.utils.len() as f64;
        assert!(len > 0.);

        let sum = self.utils.iter().map(|u| u.gpu).sum::<u32>() as f64;

        sum / (len * 100.)
    }

    /// `NvidiaMetrics::time_window()` converts the struct into TimeWindow.
    ///
    /// ### Important Note
    ///
    /// Because the sample period is unknown (between 1 second and 1/6 second), we assume it was 1 second.
    pub fn time_window(&self) -> TimeWindow {
        let start = self.timestamp;
        let end = start + TimeDelta::seconds(1);

        let start = datetime_to_prost(start);
        let end = datetime_to_prost(end);

        TimeWindow {
            start: Some(start),
            end: Some(end),
        }
    }
}

impl Into<MonitorWindow> for NvidiaMetrics {
    fn into(self) -> MonitorWindow {
        let window = Some(self.time_window());
        let utilization = Some(self.into());

        MonitorWindow {
            window,
            utilization,
        }
    }
}

impl Into<ResourceUtilization> for NvidiaMetrics {
    fn into(self) -> ResourceUtilization {
        ResourceUtilization {
            gpu: self.total_utilization_rate() as i32,
            cpu: (1.0 - self.cpu_load.idle * 100.0) as i32,
            ram_total: -1,
            ram_used: -1,
            vram_total: -1,
            vram_used: -1,
        }
    }
}
