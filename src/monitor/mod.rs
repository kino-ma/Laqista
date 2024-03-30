use std::{
    io::{BufRead, BufReader},
    process::{self, Command},
    time::SystemTime,
};

#[cfg(target_os = "macos")]
pub mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::*;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;
use tokio::sync::mpsc;

pub struct PowerMonitor {}

#[derive(Clone, Debug)]
pub struct MetricsWindow {
    start: SystemTime,
    end: SystemTime,
    metrics: PowerMetrics,
}

impl PowerMonitor {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn start(&self, tx: mpsc::Sender<MetricsWindow>) {
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
