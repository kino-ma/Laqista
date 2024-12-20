use std::{
    io::{BufRead, BufReader, Lines},
    process::{self, ChildStdout, Command},
    time::SystemTime,
};

use bytes::BytesMut;
use chrono::{DateTime, TimeDelta, Utc};
use plist::Date;
use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    proto::{MonitorWindow, ResourceUtilization, TimeWindow},
    utils::datetime_to_prost,
};

use super::SendMetrics;

pub struct MetricsMonitor {}

#[derive(Clone, Debug)]
pub struct MetricsWindow {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    metrics: PowerMetrics,
    cpu_percent: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gpu {
    pub freq_hz: f64,
    pub idle_ratio: f64,
    pub dvfm_states: Vec<DvfmState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessorMetrics {
    pub clusters: Vec<ClusterMetrics>,

    #[serde(rename = "ane_energy")]
    pub ane_mj: u16,
    #[serde(rename = "cpu_energy")]
    pub cpu_mj: u32,
    #[serde(rename = "gpu_energy")]
    pub gpu_mj: u32,
    #[serde(rename = "combined_power")]
    pub package_mw: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClusterMetrics {
    pub name: String,
    pub freq_hz: f64,
    pub dvfm_states: Vec<DvfmState>,
    pub cpus: Vec<Cpu>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cpu {
    #[serde(rename = "cpu")]
    pub cpu_id: u16,
    pub freq_hz: f64,
    pub idle_ratio: f64,
    pub dvfm_states: Vec<DvfmState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DvfmState {
    pub freq: u16,
}

impl SendMetrics for MetricsMonitor {
    fn spawn(&self, tx: mpsc::Sender<MonitorWindow>) -> JoinHandle<()> {
        tokio::spawn(async move {
            println!("start start");
            let commands = Self::commands();

            let cmd = Command::new(commands[0])
                .args(&commands[1..])
                .stdout(process::Stdio::piped())
                .spawn()
                .expect("failed to spawn monitor process");

            let stdout = cmd.stdout.expect("faile to get child's stdout");
            let reader = BufReader::new(stdout);

            let plists: MetricsReader = MetricsReader::new(reader.lines());

            for metrics in plists {
                let window = metrics.into();
                tx.send(window)
                    .await
                    .map_err(|e| format!("failed to send metrics: {}", e))
                    .expect("failed to send metrics");
            }
        })
    }
}

impl MetricsMonitor {
    pub fn new() -> Self {
        Self {}
    }

    fn commands() -> Vec<&'static str> {
        vec![
            "sudo",
            "/usr/bin/powermetrics",
            "--samplers=gpu_power,cpu_power",
            "--sample-rate=1000", // in ms
            // "--sample-count=1",
            "--format=plist",
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerMetrics {
    pub gpu: Gpu,
    pub processor: ProcessorMetrics,
    pub elapsed_ns: i64,
    pub timestamp: Date,
}

impl PowerMetrics {
    pub fn to_window(self, cpu_percent: f64) -> MetricsWindow {
        let st: SystemTime = self.timestamp.into();
        let end: DateTime<Utc> = st.into();

        let duration = TimeDelta::nanoseconds(self.elapsed_ns);
        let start = end - duration;

        MetricsWindow {
            start,
            end,
            metrics: self,
            cpu_percent,
        }
    }
}

impl Gpu {
    pub fn max_frequency(&self) -> u16 {
        self.dvfm_states
            .iter()
            .map(|state| state.freq)
            .max()
            .unwrap()
    }

    pub fn min_frequency(&self) -> u16 {
        self.dvfm_states
            .iter()
            .map(|state| state.freq)
            .min()
            .unwrap()
    }

    pub fn utilization_ratio(&self) -> f64 {
        1. - self.idle_ratio
    }
}

impl ProcessorMetrics {
    pub fn _utilization_ratio(&self) -> f64 {
        let total_cores = self.clusters.iter().map(|c| c.cpus.len()).sum::<usize>() as f64;
        let total_idle_ratio = self
            .clusters
            .iter()
            .map(|c| c.cpus.iter().map(|cpu| cpu.idle_ratio).sum::<f64>())
            .sum::<f64>();

        1. - (total_idle_ratio / total_cores)
    }
}

impl Cpu {
    pub fn freq_mhz(&self) -> f64 {
        self.freq_hz / 1e6
    }

    pub fn active_ratio(&self) -> f64 {
        1.0 - self.idle_ratio
    }
}

type StdoutLines = Lines<BufReader<ChildStdout>>;
pub struct MetricsReader {
    inner: StdoutLines,
    sys: System,
}

impl MetricsReader {
    pub fn new(lines: StdoutLines) -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
        );
        Self { inner: lines, sys }
    }

    fn parse(&self, mut buff: &[u8], cpu_percent: f64) -> Result<MetricsWindow, plist::Error> {
        if buff[0] == b'\0' {
            buff = &buff[1..];
        }
        let metrics: PowerMetrics = plist::from_bytes(buff)?;

        Ok(metrics.to_window(cpu_percent))
    }

    fn collect_cpu_usage(&mut self) -> f64 {
        self.sys.refresh_cpu_all();
        let cpus = self.sys.cpus();
        cpus.iter().map(|cpu| cpu.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64
    }
}

impl<'a> Iterator for MetricsReader {
    type Item = MetricsWindow;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buff = BytesMut::new();
        let mut have_seen_idle_ratio = false;
        let mut have_seen_gpu = false;

        while let Some(line) = self.inner.next() {
            let line = line.expect("failed to read line");

            // HACK: powermetrics has a bug to output dpulicated "idle_ratio" for only gpu_power, but not cpu_power.
            //       And we observed the processor metrics comes before the gpu metrics.
            //       So we only take idle ratio that has come first, in gpu metrics.
            //       Once Apple fixes the bug or output order has changed, this program accidentaly starts to be broken.
            if line.contains("gpu") {
                have_seen_gpu = true;
            }
            if line.starts_with("<key>idle_ratio</key>") {
                if have_seen_idle_ratio {
                    continue;
                }

                if have_seen_gpu {
                    have_seen_idle_ratio = true;
                }
            }

            buff.extend(line.as_bytes());

            if line == "</plist>" {
                let cpu_percent = self.collect_cpu_usage();

                let parsed = self.parse(&buff, cpu_percent);
                if let Err(e) = &parsed {
                    println!("WARN: failed to parse plist: {}", e);
                    println!("last data = '{}'", line);
                }
                return parsed.ok();
            }
        }

        None
    }
}

impl MetricsWindow {
    pub fn time_window(&self) -> TimeWindow {
        let start = Some(datetime_to_prost(self.start));
        let end = Some(datetime_to_prost(self.end));

        TimeWindow { start, end }
    }
}

impl Into<ResourceUtilization> for MetricsWindow {
    fn into(self) -> ResourceUtilization {
        let cpu = self.cpu_percent as i32;
        let gpu = (self.metrics.gpu.utilization_ratio() * 100.0) as i32;

        ResourceUtilization {
            gpu,
            cpu,
            ram_total: -1,
            ram_used: -1,
            vram_total: -1,
            vram_used: -1,
        }
    }
}

impl Into<MonitorWindow> for MetricsWindow {
    fn into(self) -> MonitorWindow {
        let window = Some(self.time_window());
        let utilization = Some(self.clone().into());

        MonitorWindow {
            window,
            utilization,
        }
    }
}
