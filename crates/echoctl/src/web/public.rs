//! Whether an address is on the public internet (`public`), and the one public address a host name is reached at
//! (`public_address`): what Echo checks before it reaches a host no plugin was granted.

use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};

/// A picture host's name looked up for 10 s at most (and never past the pictures' 2 minutes).
const LOOKUP_SECS: u64 = 10;

/// An address on the public internet: not this computer, the user's own network (private, link-local, shared
/// address space, unique-local), nor a reserved, documentation or multicast range.
pub(crate) fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !(a.is_private()
                || a.is_loopback()
                || a.is_link_local()
                || a.is_unspecified()
                || a.is_broadcast()
                || a.is_multicast()
                || a.is_documentation()
                || o[0] == 0
                || o[0] >= 240
                || (o[0] == 100 && (64..128).contains(&o[1]))
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || (o[0] == 198 && (18..20).contains(&o[1])))
        }
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return public(IpAddr::V4(v4));
            }
            let s = a.segments();
            // NAT64 (64:ff9b::/96) carries an IPv4 address.
            if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                return public(IpAddr::V4(std::net::Ipv4Addr::new((s[6] >> 8) as u8, s[6] as u8, (s[7] >> 8) as u8, s[7] as u8)));
            }
            // Refused whole: IPv4-compatible (::/96) and IPv4-translated (::ffff:0:0/96) forms, 6to4 (2002::/16), Teredo
            // (2001::/32), local-use NAT64 (64:ff9b:1::/48), discard-only (100::/64), site-local (fec0::/10), besides
            // loopback, unspecified, multicast, unique-local, link-local and documentation.
            !(a.is_loopback()
                || a.is_unspecified()
                || a.is_multicast()
                || s[..6] == [0; 6]
                || s[..6] == [0, 0, 0, 0, 0xffff, 0]
                || s[0] == 0x2002
                || (s[0] == 0x2001 && s[1] == 0)
                || s[..3] == [0x64, 0xff9b, 1]
                || s[..4] == [0x100, 0, 0, 0]
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || (s[0] & 0xffc0) == 0xfec0
                || (s[0] == 0x2001 && s[1] == 0xdb8))
        }
    }
}

/// `f()` on a thread of its own, given up on after `wait` (the thread ends whenever `f` does).
pub(crate) fn within<T: Send + 'static>(wait: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(wait).ok()
}

/// The one public address a picture's host is reached at: every address its name has must be public (a name that also
/// points into the user's network is refused), looked up by `until` (`LOOKUP_SECS` at most). A host written as a
/// number other than a plain IPv4 or IPv6 address (`2130706433`, `0x7f.1`) is refused, as curl would read it.
pub(crate) fn public_address(host: &str, port: u16, until: Instant) -> Result<IpAddr> {
    public_address_by(host, port, until, Arc::new(system_lookup), &crate::netif::addresses())
}

/// Whether `ip` is this computer's: one of its interfaces' addresses (`own`), or an IPv6 address in the /64 of one of
/// its global ones (the rest of its own network's prefix). An IPv4-mapped address is read as its IPv4 one.
pub(crate) fn this_computers(ip: IpAddr, own: &[IpAddr]) -> bool {
    let plain = |ip: IpAddr| match ip {
        IpAddr::V6(a) => a.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    };
    let ip = plain(ip);
    own.iter().map(|o| plain(*o)).any(|o| {
        o == ip
            || match (o, ip) {
                (IpAddr::V6(o), IpAddr::V6(a)) => public(IpAddr::V6(o)) && o.segments()[..4] == a.segments()[..4],
                _ => false,
            }
    })
}

/// How a host name's addresses are looked up: the system's resolver (`system_lookup`), or a test's.
pub(crate) type Lookup = dyn Fn(&str, u16) -> std::io::Result<Vec<IpAddr>> + Send + Sync;

/// The system's resolver.
pub(crate) fn system_lookup(host: &str, port: u16) -> std::io::Result<Vec<IpAddr>> {
    (host, port).to_socket_addrs().map(|a| a.map(|a| a.ip()).collect())
}

/// `public_address`, the name looked up by `lookup`, and this computer's own addresses `own` (`this_computers`) refused
/// too.
pub(crate) fn public_address_by(host: &str, port: u16, until: Instant, lookup: Arc<Lookup>, own: &[IpAddr]) -> Result<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if public(ip) && !this_computers(ip, own) { Ok(ip) } else { bail!("{host} is on this computer or your own network") };
    }
    let last = host.trim_end_matches('.').rsplit('.').next().unwrap_or("");
    if last.is_empty() || last.bytes().all(|b| b.is_ascii_digit()) || last.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("0x")) || !host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
        bail!("“{host}” isn't a host name Echo reaches");
    }
    let wait = until.saturating_duration_since(Instant::now()).min(Duration::from_secs(LOOKUP_SECS));
    let name = host.to_string();
    let found = if wait.is_zero() { None } else { within(wait, move || lookup(&name, port)) };
    let ips = found.ok_or_else(|| anyhow!("looking up {host} took too long"))?.unwrap_or_default();
    // A name that points at this computer reads as one into the user's network.
    let ips: Vec<IpAddr> = if ips.iter().any(|i| this_computers(*i, own)) { vec![] } else { ips };
    all_public(host, &ips)
}

/// The address to reach `host` at, of those its name has: the first, when every one is public. A name that points
/// into the user's network and one that isn't found are refused in the same words: no name inside it can be told apart.
pub(crate) fn all_public(host: &str, ips: &[IpAddr]) -> Result<IpAddr> {
    match ips.first() {
        Some(ip) if ips.iter().all(|i| public(*i)) => Ok(*ip),
        _ => bail!("Echo found no public address for {host} (never this computer or your own network)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_are_public() {
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111", "::ffff:8.8.8.8", "64:ff9b::808:808"] {
            assert!(public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.68.1", "169.254.1.1", "100.64.0.1", "0.0.0.0", "255.255.255.255", "224.0.0.1", "192.0.2.1", "198.18.0.1", "::1", "::", "fc00::1", "fd12::1", "fe80::1", "::ffff:127.0.0.1", "::ffff:192.168.1.1", "64:ff9b::7f00:1", "2001:db8::1"] {
            assert!(!public(ip.parse().unwrap()), "{ip}");
        }
        // IPv6 forms that carry or tunnel to an IPv4 address, and the old site-local range.
        for ip in ["::127.0.0.1", "::8.8.8.8", "::ffff:0:127.0.0.1", "::ffff:0:8.8.8.8", "2002:7f00:1::1", "2002:808:808::1", "2001::1", "2001:0:4136:e378:8000:63bf:3fff:fdd2", "64:ff9b:1::7f00:1", "64:ff9b:1::808:808", "fec0::1", "feff::1", "100::1"] {
            assert!(!public(ip.parse().unwrap()), "{ip}");
        }
        let soon = || Instant::now() + Duration::from_secs(10);
        for host in ["2130706433", "0x7f.1", "127.1", "evil.example\\x", "a b"] {
            assert!(public_address(host, 80, soon()).is_err(), "{host}");
        }
        assert!(public_address("127.0.0.1", 80, soon()).unwrap_err().to_string().contains("your own network"));
        assert!(public_address("localhost", 80, soon()).unwrap_err().to_string().contains("your own network"));
        // A name looked up only within the time left.
        assert!(public_address("acme.example", 443, Instant::now()).unwrap_err().to_string().contains("took too long"));
    }

    #[test]
    fn this_computers_own_addresses_and_its_ipv6_network_are_never_public() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let own = [ip("127.0.0.1"), ip("93.184.215.99"), ip("2606:2800:21f:cb07::5"), ip("fe80::1")];
        for a in ["93.184.215.99", "::ffff:93.184.215.99", "2606:2800:21f:cb07::5", "2606:2800:21f:cb07:abcd::1"] {
            assert!(this_computers(ip(a), &own), "{a}");
        }
        for a in ["93.184.215.98", "2606:2800:21f:cb08::5", "fe80::2"] {
            assert!(!this_computers(ip(a), &own), "{a}: a link-local /64 isn't shared");
        }
        let soon = Instant::now() + Duration::from_secs(10);
        let to = |ips: &'static [&'static str]| -> Arc<Lookup> { Arc::new(move |_, _| Ok(ips.iter().map(|s| s.parse().unwrap()).collect())) };
        assert!(public_address_by("93.184.215.99", 443, soon, to(&[]), &own).unwrap_err().to_string().contains("your own network"));
        assert!(public_address_by("mine.acme.example", 443, soon, to(&["2606:2800:21f:cb07::77"]), &own).unwrap_err().to_string().contains("no public address"));
        assert!(public_address_by("half.acme.example", 443, soon, to(&["93.184.215.14", "93.184.215.99"]), &own).is_err());
        assert_eq!(public_address_by("news.acme.example", 443, soon, to(&["93.184.215.14"]), &own).unwrap(), ip("93.184.215.14"));
    }

    #[test]
    fn a_name_that_takes_too_long_to_look_up_is_given_up_on() {
        let at = Instant::now();
        let slow = within(Duration::from_millis(100), || {
            std::thread::sleep(Duration::from_secs(3));
            1
        });
        assert!(slow.is_none() && at.elapsed() < Duration::from_secs(1), "{:?}", at.elapsed());
        assert_eq!(within(Duration::from_secs(5), || 2), Some(2));
    }
}
