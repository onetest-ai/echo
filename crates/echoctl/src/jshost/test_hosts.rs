//! The test hosts: in a development build, `ECHOCTL_TEST_HOSTS` sends a plugin's `ctx.http.fetch` to a fake service
//! on this computer (moved from `jshost.rs` as it is).

use anyhow::{bail, Result};

/// Development builds and tests: `ECHOCTL_TEST_HOSTS=api.github.com=http://127.0.0.1:47990,…` sends a request to
/// `https://<host>/<path>` — a host the manifest names, already checked — to `<base>/<path>`, a fake service on this
/// computer (plain http to 127.0.0.1 or localhost only). Fails closed: once it is set, a mapping that isn't a local
/// address, or a named host it doesn't map, fails the request rather than reach the real host. `Ok(None)`: the URL
/// as it is (unset, or an address the user listed). Never read by a release build.
pub(super) fn test_host(url: &str, hosts: &[String]) -> Result<Option<String>> {
    #[cfg(debug_assertions)]
    {
        match std::env::var_os("ECHOCTL_TEST_HOSTS") {
            None => Ok(None),
            Some(map) => test_host_in(map.to_str().ok_or_else(|| anyhow::anyhow!("ECHOCTL_TEST_HOSTS isn't text"))?, url, hosts),
        }
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (url, hosts);
        Ok(None)
    }
}

#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub(super) fn test_host_in(map: &str, url: &str, hosts: &[String]) -> Result<Option<String>> {
    // Every entry, up front: one that is wrong fails every request, whichever host it names.
    let mut fakes: Vec<(String, String)> = Vec::new();
    for entry in map.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let Some((host, base)) = entry.split_once('=').map(|(h, b)| (h.trim().to_ascii_lowercase(), b.trim())) else {
            bail!("ECHOCTL_TEST_HOSTS: “{entry}” isn't <host>=http://127.0.0.1:<port>");
        };
        if host.is_empty() || !host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
            bail!("ECHOCTL_TEST_HOSTS: “{host}” isn't a host name");
        }
        let Some(base) = local_base(base) else {
            bail!("ECHOCTL_TEST_HOSTS: “{base}” (for {host}) isn't a local address — http://127.0.0.1:<port> or http://localhost:<port>");
        };
        if fakes.iter().any(|(h, _)| *h == host) {
            bail!("ECHOCTL_TEST_HOSTS: {host} is mapped twice");
        }
        fakes.push((host, base));
    }
    let Some(rest) = url.strip_prefix("https://") else { return Ok(None) };
    let (host, path) = rest.find(['/', '?', '#']).map_or((rest, ""), |i| rest.split_at(i));
    if !hosts.iter().any(|h| h == host) {
        // An address the user listed (`network.hosts_from`): theirs, as it is.
        return Ok(None);
    }
    match fakes.iter().find(|(h, _)| h.eq_ignore_ascii_case(host)) {
        Some((_, base)) => Ok(Some(format!("{base}{path}"))),
        None => bail!("{host} isn't mapped in ECHOCTL_TEST_HOSTS: a test never reaches the real {host}"),
    }
}

/// `http://127.0.0.1:<port>[/path]` or `http://localhost:<port>[/path]` (scheme and name in any case), written
/// lower-case and without a trailing `/`; anything else None.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub(super) fn local_base(base: &str) -> Option<String> {
    let rest = base.get(..7).filter(|s| s.eq_ignore_ascii_case("http://")).and(base.get(7..))?;
    let (authority, path) = rest.find('/').map_or((rest, ""), |i| rest.split_at(i));
    let (name, port) = authority.split_once(':')?;
    let name = name.to_ascii_lowercase();
    let port_ok = !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit()) && port.parse::<u16>().is_ok_and(|p| p > 0);
    let path_ok = !path.bytes().any(|b| b.is_ascii_whitespace() || b.is_ascii_control() || b"@?#\\".contains(&b));
    (matches!(name.as_str(), "127.0.0.1" | "localhost") && port_ok && path_ok).then(|| format!("http://{name}:{port}{}", path.trim_end_matches('/')))
}
