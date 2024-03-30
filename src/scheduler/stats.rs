use prost_types::Timestamp;

use crate::{
    monitor::MetricsWindow,
    proto::{MonitorWindow, ResourceUtilization},
    utils::{subtract_window, IdMap},
    ServerInfo,
};

pub type StatsMap = IdMap<ServerStats>;

#[derive(Clone, Debug)]
pub struct ServerStats {
    pub server: ServerInfo,
    pub stats: Vec<MetricsWindow>,
}

impl ServerStats {
    pub fn windows(&self) -> Windows {
        let inner = self.stats.iter();
        Windows { inner }
    }

    pub fn append(&mut self, window: MonitorWindow) {
        self.stats.push(window)
    }
}

pub struct Windows<'a> {
    inner: std::slice::Iter<'a, MetricsWindow>,
}

pub struct Window {
    pub start: Timestamp,
    pub end: Timestamp,
    pub nanos: i64,
    pub utilization: ResourceUtilization,
}

impl<'a> Iterator for Windows<'a> {
    type Item = Window;
    fn next(&mut self) -> Option<Self::Item> {
        let stats = self.inner.next()?;
        let window = stats.window.as_ref().expect("Start cannot be empty");

        let start = window.start.clone().expect("Start cannot be empty");
        let end = window.end.clone().expect("End cannot be empty");
        let nanos = subtract_window(&end, &start);
        let utilization = stats
            .utilization
            .clone()
            .expect("Utilization cannot be empty");

        Some(Window {
            start,
            end,
            nanos,
            utilization,
        })
    }
}
