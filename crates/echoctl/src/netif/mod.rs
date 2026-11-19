//! This computer's own addresses: every address of every network interface (`addresses`). A host Echo reaches for a
//! plugin as "public" must not be one of them (`web::public`): a name can point at this computer's own global IPv6 or
//! public IPv4 address, which no private range catches.

// Read by the public-address check (`web::public`, feature `js`).
#![cfg_attr(not(feature = "js"), allow(dead_code))]

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use std::net::IpAddr;

/// Every address of every interface of this computer (none when they can't be read).
#[cfg(target_os = "macos")]
pub fn addresses() -> Vec<IpAddr> {
    macos::addresses()
}

#[cfg(windows)]
pub fn addresses() -> Vec<IpAddr> {
    windows::addresses()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn addresses() -> Vec<IpAddr> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn this_computers_addresses_include_its_loopback() {
        let all = super::addresses();
        assert!(all.iter().any(|a| a.is_loopback()), "{all:?}");
    }
}
