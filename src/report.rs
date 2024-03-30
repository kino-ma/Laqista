use std::error::Error;

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    monitor::{MetricsWindow, PowerMonitor},
    proto::{scheduler_client::SchedulerClient, ReportRequest},
    ServerInfo,
};

pub struct MetricsReporter {
    scheduler: ServerInfo,
    server: ServerInfo,
    rx: mpsc::Receiver<MetricsWindow>,
    monitor_handle: JoinHandle<()>,
}

impl MetricsReporter {
    pub fn new(scheduler: ServerInfo, server: ServerInfo) -> Self {
        let (tx, rx) = mpsc::channel(10);

        let monitor_handle = tokio::spawn(async move {
            println!("start thread");
            let monit = PowerMonitor::new();
            monit.start(tx).await;
        });

        Self {
            scheduler,
            server,
            rx,
            monitor_handle,
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

        println!("end listen thread");
    }

    pub async fn report(&self, metrics: &MetricsWindow) -> Result<(), Box<dyn Error>> {
        let mut client = SchedulerClient::connect(self.scheduler.addr.clone()).await?;

        let server = Some(self.server.clone().into());

        let windows = vec![metrics.clone().into()];

        let req = ReportRequest { windows, server };

        client.report(req).await?;

        Ok(())
    }
}
