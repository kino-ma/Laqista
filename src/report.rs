use std::error::Error;

use tokio::{sync::mpsc, task::JoinHandle};

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
    pub fn new(scheduler: ServerInfo, server: ServerInfo) -> Self {
        let (tx, rx) = mpsc::channel(10);

        let sender: Box<dyn SendMetrics> = Box::new(MetricsMonitor::new());
        let monitor_handle = sender.spawn(tx);

        Self {
            scheduler,
            server,
            rx,
            sender_handle: monitor_handle,
        }
    }

    pub async fn start(&mut self) {
        println!("start listen thread");

        while let Some(window) = self.rx.recv().await {
            println!("metrics window = {:?}", window);

            self.report(&window.into())
                .await
                .expect("failed to report metrics");
        }
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
