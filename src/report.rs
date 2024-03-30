use std::error::Error;

use tokio::{select, sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    monitor::{MetricsMonitor, SendMetrics},
    proto::{scheduler_client::SchedulerClient, MonitorWindow, ReportRequest},
    ServerInfo,
};

pub struct MetricsReporter {
    scheduler: ServerInfo,
    server: ServerInfo,
    rx: mpsc::Receiver<MonitorWindow>,
    sender_handle: JoinHandle<()>,
}

impl MetricsReporter {
    pub fn new(server: ServerInfo, scheduler: ServerInfo) -> Self {
        let (tx, rx) = mpsc::channel(1);

        let sender: Box<dyn SendMetrics> = Box::new(MetricsMonitor::new());
        let monitor_handle = sender.spawn(tx);

        Self {
            scheduler,
            server,
            rx,
            sender_handle: monitor_handle,
        }
    }

    pub async fn start(&mut self, token: CancellationToken) {
        println!("start listen thread");

        loop {
            select! {
                Some(window) = self.rx.recv() => {

                    self.report(&window.into())
                        .await
                        .expect("failed to report metrics");
                }
                _ = token.cancelled() => {
                    println!("cancelled");
                    self.stop();
                    break;
                }
            }
        }
    }

    pub fn stop(&mut self) {
        self.sender_handle.abort()
    }

    pub async fn report(&self, metrics: &MonitorWindow) -> Result<(), Box<dyn Error>> {
        let mut client = SchedulerClient::connect(self.scheduler.addr.clone()).await?;

        let server = Some(self.server.clone().into());

        let windows = vec![metrics.clone().into()];

        let req = ReportRequest { windows, server };

        client.report(req).await?;

        Ok(())
    }
}
