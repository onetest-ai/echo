//! The addresses a user lists in a setting named by `uses.network.hosts_from`: `scheme://host[:port]` patterns,
//! comma- or space-separated, with `*` for a number of an IPv4 address, a leading name label (`*.lab.local`, one or
//! more labels) or the port. https to any host; plain http only inside the user's own network.

use anyhow::{bail, Result};

#[derive(Debug, PartialEq)]
enum Host {
    /// Four parts, each a number or `*` (the first never `*`).
    Ip([Option<u8>; 4]),
    /// A name, `*.` in front matching one or more labels.
    Name { any_below: bool, name: String },
}

/// One pattern the user entered.
#[derive(Debug, PartialEq)]
pub struct Address {
    https: bool,
    host: Host,
    /// `None` is `:*`, any port.
    port: Option<u16>,
}

/// Every address in a setting's value; any pattern that isn't allowed refuses the whole value.
pub fn parse_list(value: &str) -> Result<Vec<Address>> {
    value.split([',', ' ', '\n', '\r', '\t']).filter(|s| !s.is_empty()).map(|p| parse(p).map_err(|e| anyhow::anyhow!("{p}: {e}"))).collect()
}

fn parse(pattern: &str) -> Result<Address> {
    let v = pattern.trim_end_matches('/').to_ascii_lowercase();
    let (https, rest) = match (v.strip_prefix("https://"), v.strip_prefix("http://")) {
        (Some(r), _) => (true, r),
        (_, Some(r)) => (false, r),
        _ => bail!("an address starts with http:// or https://"),
    };
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, "*")) => (h, None),
        Some((h, p)) => (h, Some(p.parse::<u16>().ok().filter(|n| *n > 0).ok_or_else(|| anyhow::anyhow!("the port is a number from 1 to 65535, or *"))?)),
        None => (rest, Some(if https { 443 } else { 80 })),
    };
    let host = host_pattern(host)?;
    if !https && !own_network(&host) {
        bail!("plain http is only for addresses inside your own network (192.168.x.x, 10.x.x.x, 172.16–31.x.x, localhost, *.local); use https");
    }
    Ok(Address { https, host, port })
}

fn host_pattern(h: &str) -> Result<Host> {
    const SHAPE: &str = "a host is a name or an IPv4 number, `*` only for a number of the address or the first label of a name, no path (e.g. http://192.168.1.*:8808, https://*.lab.acme.example)";
    let parts: Vec<&str> = h.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| *p == "*" || (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))) {
        if parts[0] == "*" {
            bail!("the first number of an address can't be *: that is the whole internet");
        }
        let o: Vec<Option<u8>> = parts.iter().map(|p| octet(p)).collect();
        if parts.iter().zip(&o).any(|(p, o)| *p != "*" && o.is_none()) {
            bail!("each number of an address is 0–255, without leading zeros");
        }
        return Ok(Host::Ip([o[0], o[1], o[2], o[3]]));
    }
    let any_below = parts.first() == Some(&"*");
    let name = &parts[usize::from(any_below)..];
    if name.is_empty() || !name.iter().all(|l| label(l)) {
        bail!(SHAPE);
    }
    if any_below && name.len() < 2 && name != ["local"] {
        bail!("*.{} covers a whole top-level domain: name the domain too (e.g. *.acme.example)", name.join("."));
    }
    Ok(Host::Name { any_below, name: name.join(".") })
}

/// A number 0–255 written without leading zeros (curl reads those as octal).
fn octet(p: &str) -> Option<u8> {
    (!p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()) && (p == "0" || !p.starts_with('0'))).then(|| p.parse().ok()).flatten()
}

fn label(l: &str) -> bool {
    !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// A private-range IPv4 pattern (its fixed numbers alone place it there), `localhost` or a `.local` name.
fn own_network(h: &Host) -> bool {
    match h {
        Host::Ip(o) => matches!((o[0], o[1]), (Some(10 | 127), _) | (Some(192), Some(168)) | (Some(169), Some(254))) || matches!((o[0], o[1]), (Some(172), Some(16..=31))),
        Host::Name { any_below, name } => (name == "localhost" && !any_below) || name == "local" || name.ends_with(".local"),
    }
}

impl Address {
    /// Whether `url` is on an address this pattern covers.
    pub fn matches(&self, url: &str) -> bool {
        let Some((https, host, port)) = origin(url) else { return false };
        if https != self.https || self.port.is_some_and(|p| p != port) {
            return false;
        }
        match &self.host {
            Host::Ip(pat) => {
                let o: Vec<Option<u8>> = host.split('.').map(octet).collect();
                o.len() == 4 && o.iter().zip(pat).all(|(o, p)| o.is_some() && (p.is_none() || o == p))
            }
            Host::Name { any_below, name } => {
                host.split('.').all(label) && if *any_below { host.strip_suffix(name.as_str()).is_some_and(|r| r.len() > 1 && r.ends_with('.')) } else { host == *name }
            }
        }
    }
}

/// `(https, host, port)` of a URL: no login, brackets, backslashes or escapes in its host part.
fn origin(url: &str) -> Option<(bool, String, u16)> {
    let lower = url.to_ascii_lowercase();
    let (https, rest) = match (lower.strip_prefix("https://"), lower.strip_prefix("http://")) {
        (Some(r), _) => (true, r),
        (_, Some(r)) => (false, r),
        _ => return None,
    };
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    if auth.contains(['@', '\\', '[', ']', '%']) {
        return None;
    }
    let (host, port) = match auth.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok().filter(|n| *n > 0)?),
        None => (auth, if https { 443 } else { 80 }),
    };
    Some((https, host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_lists_addresses_https_anywhere_plain_http_inside_their_network() {
        for ok in [
            "http://192.168.68.116:8808",
            "http://192.168.68.116:8808/",
            "http://192.168.68.*:*",
            "http://192.168.*.*",
            "http://10.*.*.*:8808",
            "http://172.16.*.*",
            "http://172.31.255.254",
            "http://127.0.0.1:1",
            "http://169.254.*.*",
            "http://localhost:3000",
            "http://tts.local",
            "http://*.local:*",
            "http://*.lab.local",
            "https://tts.acme.example",
            "https://*.acme.example:8443",
            "https://8.8.*.*",
        ] {
            assert!(parse_list(ok).is_ok(), "{ok}");
        }
        assert_eq!(parse_list(" http://192.168.1.*:*,https://A.example\nhttp://10.0.0.1 ").unwrap().len(), 3, "commas, spaces and lines");
        assert!(parse_list("").unwrap().is_empty());
        // Plain http to the open internet, wildcards that leave the user's network, numbers curl could read another way.
        for public in ["http://example.com", "http://8.8.8.8", "http://172.32.0.1", "http://172.*.*.*", "http://192.*.*.*", "http://192.169.1.1", "http://localhost.evil.example", "http://*.localhost", "http://010.0.0.1", "http://0x7f.0.0.1"] {
            assert!(parse(public).is_err(), "{public}");
        }
        assert!(parse("http://example.com").unwrap_err().to_string().contains("own network"), "says why");
        // Wildcards too wide, in the wrong place, or not a pattern at all.
        for bad in ["http://*.*.*.*:*", "https://*", "https://*.com", "https://a.*.example", "https://*a.example", "http://192.168.1.1*", "", "192.168.1.1", "ftp://192.168.1.1", "http://", "http://192.168.1.1:0", "http://192.168.1.1:99999", "http://192.168.1.1:x", "http://192.168.1.1/path", "http://me@192.168.1.1", "http://[::1]", "https://a b.example"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert!(parse_list("http://10.0.0.1, http://example.com").is_err(), "one bad pattern refuses the list");
    }

    #[test]
    fn an_address_covers_only_the_urls_it_names() {
        let a = |p: &str| parse(p).unwrap();
        let lan = a("http://192.168.68.*:*");
        assert!(lan.matches("http://192.168.68.5:8808/tts?x=1") && lan.matches("http://192.168.68.200") && lan.matches("HTTP://192.168.68.1/"));
        for no in ["https://192.168.68.5", "http://192.168.69.5", "http://192.168.68.05", "http://192.168.68.256", "http://192.168.68.5.evil.example", "http://192.168.68.5@evil.example", "http://evil.example#@192.168.68.5", "http://192.168.68.5:0", "http://3232252933", "http://192.168.68"] {
            assert!(!lan.matches(no), "{no}");
        }
        let port = a("http://10.0.0.1:8808");
        assert!(port.matches("http://10.0.0.1:8808/a") && !port.matches("http://10.0.0.1/a") && !port.matches("http://10.0.0.1:8809/a"));
        let default = a("http://10.0.0.1");
        assert!(default.matches("http://10.0.0.1/a") && default.matches("http://10.0.0.1:80/a") && !default.matches("http://10.0.0.1:8808/a"));
        let names = a("https://*.lab.acme.example");
        assert!(names.matches("https://gpu1.lab.acme.example/v1") && names.matches("https://a.b.lab.acme.example"));
        for no in ["https://lab.acme.example", "https://evillab.acme.example", "https://gpu1.lab.acme.example.evil.example", "http://gpu1.lab.acme.example", "https://gpu1.lab.acme.example:8443", "https://a..lab.acme.example"] {
            assert!(!names.matches(no), "{no}");
        }
        let exact = a("http://tts.local:8808");
        assert!(exact.matches("http://TTS.local:8808/x") && !exact.matches("http://a.tts.local:8808/x"));
    }
}
