//! This computer's own addresses: every address of every network interface (`addresses`). A host Echo reaches for a
//! plugin as "public" must not be one of them (`web::public`): a name can point at this computer's own global IPv6 or
//! public IPv4 address, which no private range catches. When they can't be read, that is an error: the check fails
//! closed (nothing is reached), never open with an empty list.

// Read by the public-address check (`web::public`, feature `js`).
#![cfg_attr(not(feature = "js"), allow(dead_code))]

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use std::net::IpAddr;

use anyhow::{bail, Result};

/// Every address of every interface of this computer; an error when they can't be read (or none is found: every
/// computer has its loopback).
pub fn addresses() -> Result<Vec<IpAddr>> {
    match listed() {
        Some(all) if !all.is_empty() => Ok(all),
        _ => bail!("Echo couldn't list this computer's own addresses, so it reaches no host it would have to check against them"),
    }
}

#[cfg(target_os = "macos")]
fn listed() -> Option<Vec<IpAddr>> {
    macos::addresses()
}

#[cfg(windows)]
fn listed() -> Option<Vec<IpAddr>> {
    windows::addresses()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn listed() -> Option<Vec<IpAddr>> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn this_computers_addresses_include_its_loopback() {
        let all = super::addresses().unwrap();
        assert!(all.iter().any(|a| a.is_loopback()), "{all:?}");
    }
}
