use std::error::Error;

use tokio::{select, sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    error::Error as MlessError,
    monitor::{MetricsMonitor, SendMetrics},
    proto::{scheduler_client::SchedulerClient, ClusterState, MonitorWindow, ReportRequest},
    scheduler::{mean::MeanScheduler, AuthoritativeScheduler, Cluster},
    server::DaemonState,
    ServerInfo,
};

pub struct MetricsReporter {
    scheduler: ServerInfo,
    server: ServerInfo,
    last_cluster_state: Option<ClusterState>,
    state_tx: mpsc::Sender<DaemonState>,
    rx: mpsc::Receiver<MonitorWindow>,
    sender_handle: JoinHandle<()>,
}

impl MetricsReporter {
    pub fn new(
        state_tx: mpsc::Sender<DaemonState>,
        server: ServerInfo,
        scheduler: ServerInfo,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1);

        let sender: Box<dyn SendMetrics> = Box::new(MetricsMonitor::new());
        let monitor_handle = sender.spawn(tx);

        Self {
            scheduler,
            server,
            last_cluster_state: None,
            state_tx,
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

    pub async fn report(&mut self, metrics: &MonitorWindow) -> Result<(), Box<dyn Error>> {
        let mut client = SchedulerClient::connect(self.scheduler.addr.clone()).await?;

        let server = Some(self.server.clone().into());

        let windows = vec![metrics.clone().into()];

        let req = ReportRequest { windows, server };

        let report_result = client.report(req).await;

        match report_result {
            Ok(resp) => {
                self.last_cluster_state = resp.into_inner().cluster;
                Ok(())
            }
            Err(s) => {
                let err: MlessError = s.into();
                match err {
                    MlessError::TransportError(te) => {
                        println!("MetricsReporter::report: Failed to report to the server: {te}");

                        let mean_scheduler = Box::new(MeanScheduler {});
                        let cluster_result = self
                            .last_cluster_state
                            .clone()
                            .ok_or("No latest cluster state is saved")?
                            .try_into();

                        let mut cluster: Cluster = match cluster_result {
                            Ok(cluster) => cluster,
                            Err(e) => return Err(Box::new(e)),
                        };

                        let id = cluster.group.scheduler_info.id.clone();
                        cluster.remove_server(&id);

                        let next_scheduler = cluster.choose_scheduler();

                        let state = if next_scheduler.id == self.server.id {
                            let scheduler = AuthoritativeScheduler::new(
                                cluster,
                                mean_scheduler,
                                self.state_tx.clone(),
                            );
                            DaemonState::Authoritative(scheduler)
                        } else {
                            DaemonState::Joining(next_scheduler.addr.clone())
                        };

                        self.state_tx.send(state).await?;

                        Ok(())
                    }

                    err => Err(err)?,
                }
            }
        }
    }
}
