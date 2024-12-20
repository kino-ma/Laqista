pub mod common;

#[cfg(target_os = "macos")]
pub mod apple;

#[cfg(target_os = "macos")]
pub use apple::*;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;
use tokio::{sync::mpsc, task::JoinHandle};

use crate::proto::MonitorWindow;

pub trait SendMetrics {
    fn spawn(&self, tx: mpsc::Sender<MonitorWindow>) -> JoinHandle<()>;
}
