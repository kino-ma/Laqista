use std::{
    collections::{hash_map::Iter, HashMap},
    fmt::Debug,
    time::{Duration, SystemTime},
};

use mac_address::{get_mac_address, MacAddress, MacAddressError};
use prost_types::Timestamp;
use uuid::Uuid;

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

pub fn _prost_to_system_time(timestamp: &Timestamp) -> SystemTime {
    let system_time = SystemTime::now();

    let ts_from_epoch = Duration::from_secs(timestamp.seconds as u64);
    let ts_nanos = Duration::from_nanos(timestamp.nanos as u64);

    system_time - ts_from_epoch - ts_nanos
}
