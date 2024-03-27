use std::collections::HashMap;

use prost_types::Timestamp;
use uuid::Uuid;

use crate::{
    proto::{MonitorWindow, ResourceUtilization},
    utils::subtract_window,
    ServerInfo,
};

pub type StatsMap = HashMap<Uuid, ServerStats>;

#[derive(Clone, Debug)]
pub struct ServerStats {
    pub server: ServerInfo,
    pub stats: Vec<MonitorWindow>,
}

impl ServerStats {
    pub fn windows(&self) -> Windows {
        let inner = self.stats.iter();
        Windows { inner }
    }
}

struct Windows<'a> {
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
        let window = stats.window.expect("Start cannot be empty");

        let start = window.start.expect("Start cannot be empty");
        let end = window.end.expect("End cannot be empty");
        let nanos = subtract_window(end, start);
        let utilization = stats.utilization.expect("Utilization cannot be empty");

        Some(Window {
            start,
            end,
            nanos,
            utilization,
        })
    }
}
