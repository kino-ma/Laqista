use std::{
    collections::{hash_map::Iter, HashMap},
    fmt::Debug,
};

use chrono::{DateTime, Timelike, Utc};
use mac_address::{get_mac_address, MacAddress, MacAddressError};
use prost_types::Timestamp;
use uuid::Uuid;

use crate::proto::{AppInstanceLocations, ClusterState, Group, Server};

#[derive(Clone, Debug)]
pub struct IdMap<T: Clone + Debug>(pub HashMap<Uuid, T>);

impl<T: Clone + Debug> IdMap<T> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn clone_by_ids(&self, ids: &[Uuid]) -> Self {
        let inner = ids
            .iter()
            .filter_map(|id| self.0.get(id).map(|s| (id.clone(), s.clone())))
            .collect();

        Self(inner)
    }

    pub fn iter(&self) -> Iter<Uuid, T> {
        self.0.iter()
    }
}

pub fn get_mac() -> Result<MacAddress, MacAddressError> {
    match get_mac_address() {
        Ok(Some(addr)) => Ok(addr),
        Ok(None) => Err(MacAddressError::InternalError),
        Err(err) => Err(err),
    }
}

/// subtract_window computes subtract `end` - `start`.
/// The result is returned in nanoseconds.
pub fn subtract_window(end: &Timestamp, start: &Timestamp) -> i64 {
    let mut start_i128 = i128::from(start.nanos);
    start_i128 += (end.seconds << 32) as i128;

    let mut end_i128 = i128::from(end.nanos);
    end_i128 += (end.seconds << 32) as i128;

    (end_i128 - start_i128) as i64
}

pub fn mul_as_percent(x: i64, percent: i64) -> i64 {
    let x = x as f64;
    let y = percent as f64 / 100.;

    (x * y) as i64
}

pub fn datetime_to_prost(dt: DateTime<Utc>) -> Timestamp {
    Timestamp {
        seconds: dt.second() as _,
        nanos: dt.nanosecond() as _,
    }
}

pub fn cluster_differs(a: &ClusterState, b: &ClusterState) -> bool {
    let group_changed = match (&a.group, &b.group) {
        (Some(g_a), Some(g_b)) => group_differs(&g_a, &g_b),
        (None, None) => true,
        (_, _) => false,
    };

    let servers_changed = servers_differ(&a.servers, &b.servers);

    let instances_changed = instances_differ(&a.instances, &b.instances);

    return group_changed || servers_changed || instances_changed;
}

pub fn group_differs(a: &Group, b: &Group) -> bool {
    a.scheduler
        .as_ref()
        .is_some_and(|s_a| b.scheduler.as_ref().is_some_and(|s_b| s_a.id != s_b.id))
}

pub fn servers_differ(a: &Vec<Server>, b: &Vec<Server>) -> bool {
    let mut a_ids: Vec<_> = a.iter().map(|s| &s.id).collect();
    a_ids.sort();
    let mut b_ids: Vec<_> = b.iter().map(|s| &s.id).collect();
    b_ids.sort();

    a_ids != b_ids
}

pub fn instances_differ(a: &Vec<AppInstanceLocations>, b: &Vec<AppInstanceLocations>) -> bool {
    // TODO: compare actual contents
    a.len() != b.len()
}
