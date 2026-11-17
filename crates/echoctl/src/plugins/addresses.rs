//! The addresses a user lists in a setting named by `uses.network.hosts_from`: `scheme://host[:port]` patterns,
//! comma- or space-separated, with `*` for a number of an IPv4 address, a leading name label (`*.lab.local`, one or
//! more labels) or the port. https and plain http to any host; plain http outside the user's own network is noted
//! where the list is shown (`notes`). The window's page may reach them too (`csp_sources`, the window's CSP).

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
    // `0x7f.0.0.1`, `1.2.3`: a number curl could read as an IPv4 address, not a name (no top-level domain starts with a digit).
    if name.last().is_some_and(|l| l.as_bytes()[0].is_ascii_digit()) {
        bail!("an IPv4 address is four plain numbers (e.g. 192.168.1.20); a name's last part is never a number");
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

/// A private-range IPv4 pattern (its fixed numbers alone place it there), `localhost` or a `.local` name: plain http
/// to anything else is noted (`notes`).
fn own_network(h: &Host) -> bool {
    match h {
        Host::Ip(o) => matches!((o[0], o[1]), (Some(10 | 127), _) | (Some(192), Some(168)) | (Some(169), Some(254))) || matches!((o[0], o[1]), (Some(172), Some(16..=31))),
        Host::Name { any_below, name } => (name == "localhost" && !any_below) || name == "local" || name.ends_with(".local"),
    }
}

/// What the user should know about a list as entered, one line per pattern that needs it: plain http outside their
/// own network, and an IPv4 pattern with a `*` number, which the window's page can't be let reach (a CSP source can't
/// say it). A pattern that isn't allowed is left to `parse_list`.
pub fn notes(value: &str) -> Vec<String> {
    value
        .split([',', ' ', '\n', '\r', '\t'])
        .filter(|s| !s.is_empty())
        .filter_map(|p| {
            let a = parse(p).ok()?;
            let mut why = vec![];
            if !a.https && !own_network(&a.host) {
                why.push("plain http: anyone on the way can read it");
            }
            if a.csp_sources().is_none() {
                why.push("its window part can't reach it: a * in an IPv4 address can't be allowed there (list each address)");
            }
            (!why.is_empty()).then(|| format!("{p} — {}", why.join("; ")))
        })
        .collect()
}

/// A CSP source expression the window may carry: `<http|https|ws|wss>://[*.]name-or-ipv4:<port|*>`, nothing else —
/// checked again where the policy is written, whatever built it (no `;`, space, quote, keyword or bare `*`).
pub fn csp_source_ok(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once("://") else { return false };
    let Some((host, port)) = rest.rsplit_once(':') else { return false };
    let host = host.strip_prefix("*.").unwrap_or(host);
    ["http", "https", "ws", "wss"].contains(&scheme)
        && (port == "*" || (!port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit())))
        && host.split('.').all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
}

impl Address {
    /// The CSP source expressions that let the window's page reach this address: its own scheme and the matching
    /// WebSocket one (http → ws, https → wss), on its port (`*`: any). `None` for an IPv4 pattern with a `*` number,
    /// which a CSP source can't express (only a leading name label can be a wildcard there).
    pub fn csp_sources(&self) -> Option<[String; 2]> {
        let host = match &self.host {
            Host::Ip(o) => o.iter().map(|n| n.map(|n| n.to_string())).collect::<Option<Vec<_>>>()?.join("."),
            Host::Name { any_below: true, name } => format!("*.{name}"),
            Host::Name { any_below: false, name } => name.clone(),
        };
        let port = self.port.map_or_else(|| "*".to_string(), |p| p.to_string());
        let (web, ws) = if self.https { ("https", "wss") } else { ("http", "ws") };
        Some([format!("{web}://{host}:{port}"), format!("{ws}://{host}:{port}")])
    }

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
    fn a_user_lists_addresses_https_and_plain_http_anywhere() {
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
            "http://voicecho.local:*",
            "http://*.local:*",
            "http://*.lab.local",
            "https://tts.acme.example",
            "https://*.acme.example:8443",
            "https://8.8.*.*",
            // Plain http outside the user's own network: allowed, noted where the list is shown (`notes`).
            "http://example.com",
            "http://8.8.8.8",
            "http://172.32.0.1",
            "http://localhost.evil.example",
        ] {
            assert!(parse_list(ok).is_ok(), "{ok}");
        }
        assert_eq!(parse_list(" http://192.168.1.*:*,https://A.example\nhttp://10.0.0.1 ").unwrap().len(), 3, "commas, spaces and lines");
        assert!(parse_list("").unwrap().is_empty());
        // Wildcards too wide, in the wrong place, numbers curl could read another way, or not a pattern at all.
        for bad in [
            "http://*.*.*.*:*", "https://*", "https://*.com", "http://*.localhost", "https://a.*.example", "https://*a.example", "http://192.168.1.1*",
            "http://010.0.0.1", "http://0x7f.0.0.1", "http://1.2.3", "", "192.168.1.1", "ftp://192.168.1.1", "http://", "http://192.168.1.1:0",
            "http://192.168.1.1:99999", "http://192.168.1.1:x", "http://192.168.1.1/path", "http://me@192.168.1.1", "http://[::1]", "https://a b.example",
            "ws://192.168.1.1",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert!(parse_list("http://10.0.0.1, ftp://example.com").is_err(), "one bad pattern refuses the list");
    }

    #[test]
    fn plain_http_outside_the_users_network_and_ipv4_wildcards_are_noted() {
        assert!(notes("http://voicecho.local:8808, http://*.local:*, http://10.0.0.1, http://localhost:3000, https://tts.acme.example, http://127.0.0.1:9").is_empty());
        assert_eq!(notes("http://example.com:8080"), vec!["http://example.com:8080 — plain http: anyone on the way can read it"]);
        assert_eq!(
            notes("https://8.8.*.*"),
            vec!["https://8.8.*.* — its window part can't reach it: a * in an IPv4 address can't be allowed there (list each address)"]
        );
        let both = notes("http://8.8.*.*");
        assert!(both.len() == 1 && both[0].contains("plain http") && both[0].contains("window part"), "both, on one line: {both:?}");
        assert_eq!(notes("http://192.168.1.*:*").len(), 1, "a private wildcard: only the window's note");
        assert!(notes("ftp://x.example, nonsense").is_empty(), "what isn't allowed is parse_list's to refuse");
    }

    #[test]
    fn a_listed_address_becomes_its_scheme_and_websocket_sources_for_the_window() {
        let src = |p: &str| parse(p).unwrap().csp_sources();
        let pair = |a: &str, b: &str| Some([a.to_string(), b.to_string()]);
        // The voice server on the user's network: any `.local` name, any port, plain http and its WebSocket.
        assert_eq!(src("http://*.local:*"), pair("http://*.local:*", "ws://*.local:*"));
        assert_eq!(src("http://*.local:8808"), pair("http://*.local:8808", "ws://*.local:8808"));
        assert_eq!(src("https://*.local"), pair("https://*.local:443", "wss://*.local:443"));
        assert_eq!(src("http://voicecho.local:*"), pair("http://voicecho.local:*", "ws://voicecho.local:*"));
        assert_eq!(src("http://VoiceEcho.local:8808/"), pair("http://voiceecho.local:8808", "ws://voiceecho.local:8808"));
        assert_eq!(src("http://voicecho.local"), pair("http://voicecho.local:80", "ws://voicecho.local:80"));
        assert_eq!(src("https://*.lab.acme.example:8443"), pair("https://*.lab.acme.example:8443", "wss://*.lab.acme.example:8443"));
        assert_eq!(src("http://127.0.0.1:47999"), pair("http://127.0.0.1:47999", "ws://127.0.0.1:47999"));
        assert_eq!(src("http://192.168.68.116:*"), pair("http://192.168.68.116:*", "ws://192.168.68.116:*"));
        // A `*` number of an IPv4 address: CSP can't say it, so the window isn't let reach it at all.
        for wide in ["http://192.168.68.*:*", "http://10.*.*.*:8808", "https://8.8.*.*"] {
            assert_eq!(src(wide), None, "{wide}");
        }
        for p in ["http://*.local:*", "https://*.local", "http://127.0.0.1:47999", "http://voicecho.local"] {
            assert!(src(p).unwrap().iter().all(|s| csp_source_ok(s)), "{p}");
        }
    }

    #[test]
    fn only_a_plain_source_expression_passes_the_csp_check() {
        for ok in ["http://*.local:*", "ws://*.local:*", "https://a.b.example:443", "wss://10.0.0.1:8808", "http://localhost:3000"] {
            assert!(csp_source_ok(ok), "{ok}");
        }
        for bad in [
            "*", "'self'", "'unsafe-inline'", "'unsafe-eval'", "data:", "blob:", "http:", "https://*", "http://*:*", "https://a.example",
            "http://a.example:80; script-src *", "http://a.example:80 'unsafe-eval'", "http://a.example:80'", "http://a.example:80\"",
            "http://a.example:80,http://b.example:80", "javascript://a.example:80", "ftp://a.example:21", "http://A.example:80",
            "http://a.example:80/path", "http://a..example:80", "http://*.*.example:80", "http://a.example:123456", "http://a.example:",
            "http://user@a.example:80", "http://[::1]:80", "http://a.example:80\n", "",
        ] {
            assert!(!csp_source_ok(bad), "{bad}");
        }
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
