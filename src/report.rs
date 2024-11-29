use std::error::Error;

use tokio::{select, sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    error::Error as LaqistaError,
    monitor::{MetricsMonitor, SendMetrics},
    proto::{scheduler_client::SchedulerClient, ClusterState, MonitorWindow, ReportRequest},
    scheduler::Cluster,
    server::{AppMetricReceiver, DaemonState, StateCommand, StateSender},
    utils::cluster_differs,
    ServerInfo,
};

pub struct MetricsReporter {
    scheduler: ServerInfo,
    server: ServerInfo,
    last_cluster_state: Option<ClusterState>,
    state_tx: StateSender,
    app_rx: AppMetricReceiver,
    rx: mpsc::Receiver<MonitorWindow>,
    sender_handle: JoinHandle<()>,
}

impl MetricsReporter {
    pub fn new(
        state_tx: StateSender,
        app_rx: AppMetricReceiver,
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
            app_rx,
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
                let inner = resp.into_inner();
                self.put_cluster(inner.cluster);
                Ok(())
            }
            Err(s) => {
                let err: LaqistaError = s.into();
                match err {
                    LaqistaError::TransportError(te) => {
                        println!("MetricsReporter::report: Failed to report to the server: {te}");

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

                        let state_command = if next_scheduler.id == self.server.id {
                            StateCommand::BecomeScheduler(cluster)
                        } else {
                            let state = DaemonState::Joining(next_scheduler.addr.clone());
                            StateCommand::Update(state)
                        };

                        self.state_tx.send(state_command).await?;

                        Ok(())
                    }

                    err => Err(err)?,
                }
            }
        }
    }

    fn put_cluster(&mut self, current: Option<ClusterState>) -> bool {
        let changed = match (&self.last_cluster_state, &current) {
            (Some(last), Some(current)) => cluster_differs(last, current),
            (None, None) => false,
            _ => true,
        };

        if changed {
            println!("Cluster state updated.\n{:?}", &current);
            self.last_cluster_state = current;
        }

        changed
    }
}
