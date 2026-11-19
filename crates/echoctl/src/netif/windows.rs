//! Windows: the adapters' unicast addresses from `GetAdaptersAddresses`.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use windows::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
use windows::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6};

/// `None` when `GetAdaptersAddresses` fails.
pub fn addresses() -> Option<Vec<IpAddr>> {
    let mut out = vec![];
    let mut size: u32 = 16 << 10;
    for _ in 0..4 {
        // u64s: the list's structures need 8-byte alignment.
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let first = buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        let r = unsafe { GetAdaptersAddresses(u32::from(AF_UNSPEC.0), flags, None, Some(first), &mut size) };
        if r == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if r != 0 {
            return None;
        }
        unsafe {
            let mut adapter = first;
            while !adapter.is_null() {
                let mut u = (*adapter).FirstUnicastAddress;
                while !u.is_null() {
                    let sa = (*u).Address.lpSockaddr;
                    if !sa.is_null() {
                        if (*sa).sa_family == AF_INET {
                            out.push(IpAddr::V4(Ipv4Addr::from(u32::from_be((*sa.cast::<SOCKADDR_IN>()).sin_addr.S_un.S_addr))));
                        } else if (*sa).sa_family == AF_INET6 {
                            out.push(IpAddr::V6(Ipv6Addr::from((*sa.cast::<SOCKADDR_IN6>()).sin6_addr.u.Byte)));
                        }
                    }
                    u = (*u).Next;
                }
                adapter = (*adapter).Next;
            }
        }
        return Some(out);
    }
    None
}
