use mac_address::{get_mac_address, MacAddress, MacAddressError};
use prost_types::Timestamp;

pub fn get_mac() -> Result<MacAddress, MacAddressError> {
    match get_mac_address() {
        Ok(Some(addr)) => Ok(addr),
        Ok(None) => Err(MacAddressError::InternalError),
        Err(err) => Err(err),
    }
}

/// subtract_window computes subtract `end` - `start`.
/// The result is returned in nanoseconds.
pub fn subtract_window(end: Timestamp, start: Timestamp) -> i64 {
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
