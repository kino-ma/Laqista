use std::{collections::HashMap, error::Error, time::Duration};

use laqista_core::AppRpc;
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
                    for _ in 0..3 {
                        match self.report(&window.clone().into())
                            .await {
                                Ok(_) => break,
                                Err(e) => println!("Error sending metrics: '{e:?}'. Retrying in 200 ms..."),
                            }
                            tokio::time::sleep(Duration::from_millis(200)).await;
                    }
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
        let server = Some(self.server.clone().into());

        let windows = vec![metrics.clone().into()];

        let mut app_latencies = HashMap::new();
        while let Ok(metric) = self.app_rx.try_recv() {
            let rpc = AppRpc::new(&metric.app, &metric.service, &metric.rpc);
            app_latencies.insert(rpc.to_string(), metric.elapsed.as_millis() as _);
        }

        let req = ReportRequest {
            windows,
            server,
            app_latencies,
        };

        let addr = self.scheduler.addr.clone();
        let mut counter = 0;
        let mut client = loop {
            let result = SchedulerClient::connect(addr.clone())
                .await
                .map_err(|e| LaqistaError::from(e));

            counter += 1;
            if result.is_ok() || counter >= 3 {
                break result;
            }
        }?;

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
