use std::{
    io::{BufRead, BufReader, Lines},
    process::{self, ChildStdout, Command},
    time::{Duration, SystemTime},
};

use tokio::{sync::mpsc, task::JoinHandle};

use crate::proto::{MonitorWindow, ResourceUtilization, TimeWindow};

use crate::monitor::SendMetrics;

// exmaple output: 1529693401.317127: gpu 0.00%, ee 0.00%, vgt 0.00%, ta 0.00%, sx 0.00%, sh 0.00%, spi 0.00%, sc 0.00%, pa 0.00%, db 0.00%, cb 0.00%, vram 0.04% 2.06mb, gtt 0.04% 2.56mb

pub struct MetricsMonitor {}

#[derive(Clone, Debug)]
pub struct RadeonMetrics {
    pub time: SystemTime,
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

impl MetricsMonitor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn commands() -> Vec<&'static str> {
        vec!["radeontop", "--dump", "-"]
    }
}

impl RadeonMetrics {
    pub fn time_window(&self) -> TimeWindow {
        let start = self.time;
        let end = start + Duration::from_secs(1);

        TimeWindow {
            start: Some(start.into()),
            end: Some(end.into()),
        }
    }
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

            let reader: MetricsReader = MetricsReader::new(reader.lines());

            for metrics in reader {
                let window = metrics.into();
                tx.send(window)
                    .await
                    .map_err(|e| format!("failed to send metrics: {}", e))
                    .expect("failed to send metrics");
            }
        })
    }
}

impl Into<MonitorWindow> for RadeonMetrics {
    fn into(self) -> MonitorWindow {
        let window = Some(self.time_window());
        let utilization = Some(self.into());

        MonitorWindow {
            window,
            utilization,
        }
    }
}

type StdoutLines = Lines<BufReader<ChildStdout>>;
struct MetricsReader {
    inner: StdoutLines,
}

impl MetricsReader {
    pub fn new(lines: StdoutLines) -> Self {
        Self { inner: lines }
    }
}

impl Iterator for MetricsReader {
    type Item = RadeonMetrics;
    fn next(&mut self) -> Option<Self::Item> {
        let line = self.inner.next()?.ok()?;

        radeon_top(line)
    }
}

impl Into<ResourceUtilization> for RadeonMetrics {
    fn into(self) -> ResourceUtilization {
        ResourceUtilization {
            gpu: (self.gpu * 100.) as _,
            cpu: -1,
            ram_total: -1,
            ram_used: -1,
            vram_total: -1,
            vram_used: -1,
        }
    }
}
