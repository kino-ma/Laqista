use std::{
    io::{BufRead, BufReader, Lines},
    process::{self, ChildStdout, Command},
    time::{Duration, SystemTime},
};

use bytes::BytesMut;
use plist::Date;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::proto::{MonitorWindow, ResourceUtilization, TimeWindow};

use super::SendMetrics;

// exmaple output: 1529693401.317127: gpu 0.00%, ee 0.00%, vgt 0.00%, ta 0.00%, sx 0.00%, sh 0.00%, spi 0.00%, sc 0.00%, pa 0.00%, db 0.00%, cb 0.00%, vram 0.04% 2.06mb, gtt 0.04% 2.56mb

pub struct MetricsMonitor {}

#[derive(Clone, Debug)]
pub struct MetricsWindow {
    start: SystemTime,
    end: SystemTime,
    metrics: RadeonMetrics,
}

pub struct RadeonMetrics {
    pub gpu: f64,
    pub ee: f64,
    pub vgt: f64,
    pub ta: f64,
    pub sx: f64,
    pub sh: f64,
    pub spi: f64,
    pub sc: f64,
    pub pa: f64,
    pub db: f64,
    pub cb: f64,
    pub vram: f64,
    pub git: f64,
}

impl FromStr for RadeonMetrics {
    // fn
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
            "/usr/bin/powermetrics",
            "--sampler=gpu_power",
            "--sample-rate=1000", // in ms
            // "--sample-count=1",
            "--format=plist",
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerMetrics {
    pub gpu: Gpu,
    pub elapsed_ns: i64,
    pub timestamp: Date,
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

impl Into<MetricsWindow> for PowerMetrics {
    fn into(self) -> MetricsWindow {
        let end = self.timestamp.into();
        let duration = Duration::from_nanos(self.elapsed_ns as u64);
        let start = end - duration;

        MetricsWindow {
            start,
            end,
            metrics: self,
        }
    }
}

type StdoutLines = Lines<BufReader<ChildStdout>>;
pub struct MetricsReader {
    inner: StdoutLines,
}

impl MetricsReader {
    pub fn new(lines: StdoutLines) -> Self {
        Self { inner: lines }
    }

    fn parse(&self, mut buff: &[u8]) -> Result<MetricsWindow, plist::Error> {
        if buff[0] == b'\0' {
            buff = &buff[1..];
        }
        let metrics: PowerMetrics = plist::from_bytes(buff)?;

        Ok(metrics.into())
    }
}

impl<'a> Iterator for MetricsReader {
    type Item = MetricsWindow;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buff = BytesMut::new();
        let mut have_seen_idle_ratio = false;

        while let Some(line) = self.inner.next() {
            let line = line.expect("failed to read line");

            if line.starts_with("<key>idle_ratio</key>") {
                if have_seen_idle_ratio {
                    continue;
                } else {
                    have_seen_idle_ratio = true;
                }
            }

            buff.extend(line.as_bytes());

            if line == "</plist>" {
                let parsed = self.parse(&buff);
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
        let start = Some(self.start.into());
        let end = Some(self.end.into());

        TimeWindow { start, end }
    }
}

impl Into<ResourceUtilization> for PowerMetrics {
    fn into(self) -> ResourceUtilization {
        let gpu = (self.gpu.utilization_ratio() * 100.0) as i32;

        ResourceUtilization {
            gpu,
            cpu: -1,
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
        let utilization = Some(self.metrics.into());

        MonitorWindow {
            window,
            utilization,
        }
    }
}
