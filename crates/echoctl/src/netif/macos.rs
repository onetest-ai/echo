//! macOS: the interfaces' addresses from `getifaddrs`.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// `None` when `getifaddrs` fails.
pub fn addresses() -> Option<Vec<IpAddr>> {
    let mut out = vec![];
    unsafe {
        let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut list) != 0 {
            return None;
        }
        let mut at = list;
        while !at.is_null() {
            let a = (*at).ifa_addr;
            if !a.is_null() {
                match i32::from((*a).sa_family) {
                    libc::AF_INET => out.push(IpAddr::V4(Ipv4Addr::from(u32::from_be((*a.cast::<libc::sockaddr_in>()).sin_addr.s_addr)))),
                    libc::AF_INET6 => out.push(IpAddr::V6(Ipv6Addr::from((*a.cast::<libc::sockaddr_in6>()).sin6_addr.s6_addr))),
                    _ => {}
                }
            }
            at = (*at).ifa_next;
        }
        libc::freeifaddrs(list);
    }
    Some(out)
}
