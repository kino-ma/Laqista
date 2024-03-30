use prost_types::Timestamp;

use crate::{
    proto::{MonitorWindow, ResourceUtilization},
    utils::{subtract_window, IdMap},
    ServerInfo,
};

pub type StatsMap = IdMap<ServerStats>;

#[derive(Clone, Debug)]
pub struct ServerStats {
    pub server: ServerInfo,
    pub stats: Vec<MonitorWindow>,
}

impl ServerStats {
    pub fn new(server: ServerInfo) -> Self {
        let stats = Vec::new();
        Self { server, stats }
    }

    pub fn from_stats(server: ServerInfo, stats: Vec<MonitorWindow>) -> Self {
        Self { server, stats }
    }

    pub fn windows(&self) -> Windows {
        let inner = self.stats.iter();
        Windows { inner }
    }

    pub fn append(&mut self, mut window: Vec<MonitorWindow>) {
        self.stats.append(&mut window)
    }
}

pub struct Windows<'a> {
    inner: std::slice::Iter<'a, MonitorWindow>,
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
