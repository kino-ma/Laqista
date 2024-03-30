use std::{
    io::{BufRead, BufReader, Lines},
    marker::PhantomData,
    process::{self, ChildStdout, Command},
    time::SystemTime,
};

use bytes::BytesMut;
use plist::Date;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    proto::{MonitorWindow, ResourceUtilization, TimeWindow},
    utils::prost_to_system_time,
};

pub struct PowerMonitor {}

#[derive(Clone, Debug)]
pub struct MetricsWindow {
    start: SystemTime,
    end: SystemTime,
    metrics: PowerMetrics,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerMetrics {
    pub gpu: Gpu,
    pub elapesd_ns: i64,
    pub timestamp: Date,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gpu {
    pub freq_hz: f64,
    pub idle_ratio: f64,
    pub dvfm_states: Vec<DvfmState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DvfmState {
    pub freq: u16,
}

impl PowerMonitor {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn start(&self, tx: mpsc::Sender<PowerMetrics>) {
        println!("start start");
        let commands = Self::commands();

        let cmd = Command::new(commands[0])
            .args(&commands[1..])
            .stdout(process::Stdio::piped())
            .spawn()
            .expect("failed to spawn monitor process");

        let stdout = cmd.stdout.expect("faile to get child's stdout");
        let reader = BufReader::new(stdout);

        let plists: Plists<PowerMetrics> = Plists::new(reader.lines());

        for metrics in plists {
            tx.send(metrics).await.expect("failed to send metrics");
        }
    }

    fn commands() -> Vec<&'static str> {
        vec![
            "/usr/bin/powermetrics",
            "--sampler=gpu_power",
            "--sample-rate=3000", // in ms
            // "--sample-count=1",
            "--format=plist",
        ]
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

type StdoutLines = Lines<BufReader<ChildStdout>>;
struct Plists<T> {
    inner: StdoutLines,
    phantom: PhantomData<T>,
}

impl<T> Plists<T>
where
    T: DeserializeOwned,
{
    pub fn new(lines: StdoutLines) -> Self {
        Self {
            inner: lines,
            phantom: PhantomData,
        }
    }

    fn parse(&self, mut buff: &[u8]) -> Result<T, plist::Error> {
        if buff[0] == b'\0' {
            buff = &buff[1..];
        }
        plist::from_bytes(buff)
    }
}

impl<'a, T> Iterator for Plists<T>
where
    T: DeserializeOwned,
{
    type Item = T;

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

impl TryFrom<MonitorWindow> for MetricsWindow {
    type Error = String;

    fn try_from(monitor_window: MonitorWindow) -> Result<Self, Self::Error> {
        let window = monitor_window
            .window
            .ok_or("window cannot be empty".to_owned())?;

        let start = window
            .start
            .as_ref()
            .map(prost_to_system_time)
            .ok_or("start cannot be empty".to_owned())?;

        let end = window
            .end
            .as_ref()
            .map(prost_to_system_time)
            .ok_or("end cannot be empty".to_owned())?;

        let u = monitor_window
            .utilization
            .ok_or("metrics cannot be empty")?;
        let metrics = PowerMetrics { gpu: u.gpu, elapesd_ns: (), timestamp: () }

        Ok(Self {
            start,
            end,
            metrics,
        })
    }
}
