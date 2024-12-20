use std::{
    io::{BufRead, BufReader, Error as IOError, Lines},
    process::{self, ChildStdout, Command},
};

use chrono::{DateTime, TimeDelta, Utc};
use sysinfo::System;
use tokio::sync::mpsc;

use crate::{
    monitor::common::collect_cpu_usage,
    proto::{MonitorWindow, ResourceUtilization, TimeWindow},
    utils::datetime_to_prost,
};

use super::parse::{header_line, metrics_line};

pub struct RadeonMonitor {}

impl RadeonMonitor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn commands() -> Vec<&'static str> {
        vec!["radeontop", "--dump", "-"]
    }

    pub async fn run(&self, tx: mpsc::Sender<MonitorWindow>) -> ! {
        println!("start start");
        let commands = Self::commands();

        let cmd = Command::new(commands[0])
            .args(&commands[1..])
            .stdout(process::Stdio::piped())
            .spawn()
            .expect("failed to spawn monitor process");

        let stdout = cmd.stdout.expect("faile to get child's stdout");
        let reader = BufReader::new(stdout);

        let mut reader: MetricsReader = MetricsReader::new(reader.lines());
        reader.skip_header();

        for metrics in reader {
            let window = metrics.into();
            tx.send(window)
                .await
                .map_err(|e| format!("failed to send metrics: {}", e))
                .expect("failed to send metrics");
        }

        unreachable!()
    }
}

#[derive(Clone, Debug)]
pub struct RadeonMetrics {
    cpu_percent: f64,
    gpu: RadeonGpuMetrics,
}

#[derive(Clone, Debug)]
pub struct RadeonGpuMetrics {
    pub timestamp: DateTime<Utc>,
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
    pub gtt: f64,
}

impl RadeonMetrics {
    pub fn time_window(&self) -> TimeWindow {
        let start = self.gpu.timestamp;
        let end = start + TimeDelta::seconds(1);

        let start = datetime_to_prost(start);
        let end = datetime_to_prost(end);

        TimeWindow {
            start: Some(start),
            end: Some(end),
        }
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
    sys: System,
    seen_header: bool,
}

impl MetricsReader {
    pub fn new(lines: StdoutLines) -> Self {
        let sys = System::new();
        Self {
            inner: lines,
            sys,
            seen_header: false,
        }
    }

    pub fn skip_header(&mut self) -> String {
        if self.seen_header {
            println!("WARN: MetricsReader.skip_header(): we have already seen a header");
        }

        let line = self.next_inner().expect("EOF");
        header_line(&line).expect("attempt to skip a non-header line");

        // On successful parse, return the original string directly
        line
    }

    fn next_inner(&mut self) -> Option<String> {
        let read_result = self
            .inner
            .next()
            .unwrap_or(Err(IOError::other("unexpected end of lines")));

        read_result
            .map_err(|e| println!("ERR: MetricsReader.next_inner(): failed to read line: {e}"))
            .ok()
    }
}

impl Iterator for MetricsReader {
    type Item = RadeonMetrics;
    fn next(&mut self) -> Option<Self::Item> {
        let line = self.next_inner()?;
        let (_, metrics) = metrics_line(&line)
            .map_err(|e| println!("ERR: MetricsReader.next(): failed to parse: {e}"))
            .ok()?;

        let cpu_percent = collect_cpu_usage(&mut self.sys);

        Some(RadeonMetrics {
            cpu_percent,
            gpu: metrics,
        })
    }
}

impl Into<ResourceUtilization> for RadeonMetrics {
    fn into(self) -> ResourceUtilization {
        let gpu = (self.gpu.gpu * 100.) as _;
        let cpu = self.cpu_percent as _;
        ResourceUtilization {
            cpu,
            gpu,
            ram_total: -1,
            ram_used: -1,
            vram_total: -1,
            vram_used: -1,
        }
    }
}
