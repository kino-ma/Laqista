use std::{
    io::{BufRead, BufReader, Lines},
    marker::PhantomData,
    process::{self, ChildStdout, Command},
};

use bytes::BytesMut;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct PowerMonitor {}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerMetrics {
    pub gpu: Gpu,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Gpu {
    pub freq_hz: f64,
    pub dvfm_states: Vec<DvfmState>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct DvfmState {
    pub freq: u16,
}

impl PowerMonitor {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn start(&self, tx: mpsc::Sender<PowerMonitor>) {
        println!("start start");
        let commands = Self::commands();

        loop {
            println!("loop...");
            let cmd = Command::new(commands[0])
                .args(&commands[1..])
                .stdout(process::Stdio::piped())
                .spawn()
                .expect("failed to spawn monitor process");

            let stdout = cmd.stdout.expect("faile to get child's stdout");
            let reader = BufReader::new(stdout);

            let plists: Plists<PowerMetrics> = Plists::new(reader.lines());

            for metrics in plists {
                println!("metrics = {:?}", metrics);
                println!("utilization = {:?}", metrics.gpu.utilization_ratio());
            }
            println!("end loop");
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

    pub(crate) fn min_frequency(&self) -> u16 {
        self.dvfm_states
            .iter()
            .map(|state| state.freq)
            .min()
            .unwrap()
    }

    pub fn utilization_ratio(&self) -> f64 {
        let min = self.min_frequency() as f64;
        let max = self.max_frequency() as f64;
        ((self.freq_hz - min).max(0.0) / (max - min).max(1.0))
            .max(0.0)
            .min(1.0)
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

        while let Some(line) = self.inner.next() {
            let line = line.expect("failed to read line");

            println!(
                "line = '{}' (offset 1 = {:?})",
                line,
                line.chars().take(2).collect::<Vec<_>>()
            );

            buff.extend(line.as_bytes());

            if line == "</plist>" {
                let parsed = self.parse(&buff);
                if let Err(e) = &parsed {
                    println!("WARN: failed to parse plist: {}", e);
                    println!("data = '{}'", line);
                }

                return parsed.ok();
            }
        }

        None
    }
}