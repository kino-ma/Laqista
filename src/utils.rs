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

pub fn rpc_path(package: &str, service: &str, rpc: &str) -> String {
    format!("/{package}.{service}/{rpc}")
}

pub fn parse_rpc_path(path: &str) -> Option<(&str, &str, &str)> {
    let mut paths = path.split("/").skip(1);
    let pkg_svc = paths.next()?;
    let rpc = paths.next()?;

    let mut iter = pkg_svc.split(".");
    let pkg = iter.next()?;
    let svc = iter.next()?;

    Some((pkg, svc, rpc))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_clone_by_ids() {
        let mut map = IdMap::<usize>::new();

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();

        map.0.insert(id_a, 0xa);
        map.0.insert(id_b, 0xb);
        map.0.insert(id_c, 0xc);

        let ids = vec![id_a, id_b];

        let cloned = map.clone_by_ids(&ids);

        println!("{:?}", cloned);
        assert_eq!(cloned.0.len(), ids.len());
    }

    #[test]
    fn test_rpc_path() {
        let path = "/laqista.Scheduler/Deploy";
        let expected = ("laqista", "Scheduler", "Deploy");
        let (package, service, rpc) = expected;

        let generated = rpc_path(package, service, rpc);

        assert_eq!(generated, path);

        let parsed = parse_rpc_path(&generated).unwrap();

        assert_eq!(parsed, expected);
    }
}
