use mac_address::{get_mac_address, MacAddress, MacAddressError};

pub fn get_mac() -> Result<MacAddress, MacAddressError> {
    match get_mac_address() {
        Ok(Some(addr)) => Ok(addr),
        Ok(None) => Err(MacAddressError::InternalError),
        Err(err) => Err(err),
    }
}
