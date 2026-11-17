//! Certificates the user trusts for a plugin's addresses (`network.hosts_from`): a server of their own with a
//! self-signed or internal-CA certificate is reached over https / wss once the user pinned its key — on their yes, in
//! the plugin's settings (a dialog of the system's) or with `echoctl plugin trust`. Kept outside the plugin's store, as
//! the addresses are: `<config dir>/plugins/pins/<id>.json`, `{pins: {"host:port": {key, at}}, seen: {…}}`. `seen`
//! is what Echo met on a connection that failed (the key the user may trust); a plugin can make Echo meet a key only by
//! connecting to an address the user listed, and never pin one.
//!
//! A key is `sha256//<base64>`: the SHA-256 of the certificate's public key (SubjectPublicKeyInfo), as curl's
//! `--pinnedpubkey` takes it — the same check for `ctx.http.fetch` / `stream` (curl) and `ctx.net.websocket` (rustls).

use super::*;

/// Keys Echo keeps as met, per plugin: an address listed with a `*` can meet many.
const SEEN_MOST: usize = 20;

/// The file of plugin `id`'s pins: refused for anything but a plugin id (it names the file; `../../ai` would be
/// another of Echo's), whoever asks — the window, the CLI or Echo itself.
fn file(id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        bail!("“{id}” isn't a plugin");
    }
    Ok(plugins_dir().join("pins").join(format!("{id}.json")))
}

fn read(id: &str) -> Result<Value> {
    let v = Value::Object(read_map(&file(id)?));
    Ok(json!({"pins": v["pins"].as_object().cloned().unwrap_or_default(), "seen": v["seen"].as_object().cloned().unwrap_or_default()}))
}

fn write(id: &str, v: &Value) -> Result<()> {
    let f = file(id)?;
    fs::create_dir_all(f.parent().unwrap())?;
    paths::write_atomic(&f, serde_json::to_string_pretty(v)?.as_bytes())?;
    Ok(())
}

/// `host:port` of an address as the user or a URL writes it (`https://h:p/…`, `wss://h`, `h:p`, `h`), lower case,
/// the port 443 when none is given.
pub fn host_port(address: &str) -> Result<String> {
    let a = address.trim().to_ascii_lowercase();
    let rest = a.split_once("://").map_or(a.as_str(), |(_, r)| r);
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (host, port) = match auth.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok().filter(|n| *n > 0).ok_or_else(|| anyhow!("“{address}”: the port is a number from 1 to 65535"))?),
        None => (auth, 443),
    };
    if host.is_empty() || !host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
        bail!("“{address}” isn't a host (a name or an IPv4 address, and a port)");
    }
    Ok(format!("{host}:{port}"))
}

/// A key as Echo writes it: `sha256//` and the base64 of 32 bytes.
pub fn is_key(k: &str) -> bool {
    use base64::Engine as _;
    k.strip_prefix("sha256//").and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok()).is_some_and(|b| b.len() == 32)
}

/// The key the user pinned for `host:port` of plugin `id`.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn pinned(id: &str, hp: &str) -> Option<String> {
    read(id).ok()?["pins"][hp]["key"].as_str().map(str::to_owned)
}

/// A key Echo met at `host:port` on a connection that failed (untrusted, or not the one pinned): kept for the user to
/// trust. Only Echo calls it, for an address the user listed.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn saw(id: &str, hp: &str, key: &str) -> Result<()> {
    file(id)?;
    let _lock = wait_lock(&paths::config_dir().join("plugins.lock"))?;
    let mut v = read(id)?;
    if v["seen"][hp]["key"].as_str() == Some(key) {
        return Ok(());
    }
    // Another key than the one met before: `trust` without a key no longer knows which one the user saw.
    let several = v["seen"][hp]["several"].as_bool() == Some(true) || v["seen"][hp]["key"].is_string();
    let seen = v["seen"].as_object_mut().unwrap();
    seen.insert(hp.into(), json!({"key": key, "at": chrono::Local::now().to_rfc3339(), "several": several}));
    while seen.len() > SEEN_MOST {
        let oldest = seen.iter().min_by_key(|(_, s)| s["at"].as_str().unwrap_or("").to_string()).map(|(k, _)| k.clone()).unwrap_or_default();
        seen.remove(&oldest);
    }
    write(id, &v)
}

/// Whether `host:port` is covered by an https address the user listed for plugin `id` (only those take a pin).
fn listed_https(id: &str, hp: &str) -> Result<bool> {
    let m = installed_manifest(id)?;
    let url = format!("https://{hp}/");
    Ok(address_settings(&m.uses).iter().filter_map(|k| listed_addresses(id, k)).filter_map(|v| addresses::parse_list(&v).ok()).flatten().any(|a| a.matches(&url)))
}

/// The user trusts `key` (or, none given, the one Echo met there — refused when it met more than one there since the
/// user last trusted or forgot one: they may have seen another) for `address` of plugin `id`.
pub fn trust(id: &str, address: &str, key: Option<&str>) -> Result<String> {
    file(id)?;
    let hp = host_port(address)?;
    let name = installed_manifest(id)?.name;
    if !listed_https(id, &hp)? {
        bail!("{hp} isn't an https address you listed for {name}: list it in its settings first");
    }
    let _lock = wait_lock(&paths::config_dir().join("plugins.lock"))?;
    let mut v = read(id)?;
    let key = match key.map(str::trim) {
        Some(k) if !is_key(k) => bail!("“{k}” isn't a key's fingerprint (sha256// and 44 characters of base64)"),
        Some(k) => k.to_string(),
        None if v["seen"][&hp]["several"].as_bool() == Some(true) => {
            bail!("Echo met more than one key at {hp}: give the one you trust with --key (`echoctl plugin certs {id}` shows the last one met)")
        }
        None => v["seen"][&hp]["key"].as_str().map(str::to_owned).ok_or_else(|| anyhow!("Echo hasn't met a certificate at {hp} yet: let {name} connect once, or give its key's fingerprint"))?,
    };
    v["pins"][&hp] = json!({"key": key, "at": chrono::Local::now().to_rfc3339()});
    v["seen"].as_object_mut().unwrap().remove(&hp);
    write(id, &v)?;
    Ok(key)
}

/// The user no longer trusts what they pinned at `address` (and forgets a key met there).
pub fn forget(id: &str, address: &str) -> Result<bool> {
    file(id)?;
    let hp = host_port(address)?;
    let _lock = wait_lock(&paths::config_dir().join("plugins.lock"))?;
    let mut v = read(id)?;
    let had = v["pins"].as_object_mut().unwrap().remove(&hp).is_some() | v["seen"].as_object_mut().unwrap().remove(&hp).is_some();
    write(id, &v)?;
    Ok(had)
}

/// Each address with a pin or a key met: `[{address, pinned, seen, changed}]` (`changed`: met another key than the
/// one pinned), for the settings row and `echoctl plugin certs`.
pub fn view(id: &str) -> Result<Vec<Value>> {
    let v = read(id)?;
    let mut at: Vec<String> = v["pins"].as_object().into_iter().chain(v["seen"].as_object()).flat_map(|m| m.keys().cloned()).collect();
    at.sort();
    at.dedup();
    Ok(at
        .into_iter()
        .map(|hp| {
            let (pinned, seen) = (v["pins"][&hp]["key"].clone(), v["seen"][&hp]["key"].clone());
            json!({"address": hp, "pinned": pinned, "seen": seen, "changed": !pinned.is_null() && !seen.is_null()})
        })
        .collect())
}

/// Its pins and keys met go with the plugin's data.
pub(super) fn remove_plugin(id: &str) {
    if let Ok(f) = file(id) {
        let _ = fs::remove_file(f);
    }
}

/// The window's `cert_trust` / `cert_forget`. Trusting asks the user in a dialog of the system's (the page can't be
/// told apart from a plugin's own window half), and only for the key Echo met there; forgetting narrows, no dialog.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(super) fn page_op(v: &Value) -> Result<()> {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let (id, address) = (s("id"), s("address"));
    file(&id)?;
    if s("op") == "cert_forget" {
        return forget(&id, &address).map(|_| ());
    }
    let hp = host_port(&address)?;
    let key = s("key");
    if read(&id)?["seen"][&hp]["key"].as_str() != Some(key.as_str()) {
        bail!("Not trusted: Echo met another certificate at {hp} meanwhile — look again");
    }
    if BUSY.lock().is_ok_and(|b| b.contains_key(&format!("plugin:{id}"))) {
        bail!("Not trusted: {id} is busy (being installed, updated or asking you); try again in a moment");
    }
    if refused_lately(&id) {
        bail!("Not trusted: you said no a moment ago");
    }
    let name = installed_manifest(&id)?.name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>();
    background(format!("plugin:{id}"), "Waiting for you to trust the certificate…", move || {
        if !allow_pin(&name, &hp, &key) {
            REFUSED.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), std::time::Instant::now());
            bail!("Not trusted: you didn't let {name} trust the certificate at {hp}");
        }
        trust(&id, &hp, Some(&key)).map(|_| ())
    });
    Ok(())
}

/// The user's yes, in a dialog of the system's. Development builds and tests: `ECHOCTL_ALLOW_PINS=yes|no` answers
/// instead (never in a release build).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
fn allow_pin(name: &str, hp: &str, key: &str) -> bool {
    #[cfg(debug_assertions)]
    if let Ok(a) = std::env::var("ECHOCTL_ALLOW_PINS") {
        return a == "yes";
    }
    let msg = format!("Let {name} trust the certificate of {hp}?\n\nIts key: {key}\n\nTrust it only if it is your own server's: Echo then reaches it with this key alone, and refuses another until you trust that one.");
    crate::app::ask(&msg, &["Don't trust", "Trust"], "Don't trust").as_deref() == Some("Trust")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_written_as_host_and_port_and_a_key_as_curl_pins_it() {
        assert_eq!(host_port("https://Voicecho.local").unwrap(), "voicecho.local:443");
        assert_eq!(host_port("wss://127.0.0.1:8443/rpc?x=1").unwrap(), "127.0.0.1:8443");
        assert_eq!(host_port("voicecho.local:9000").unwrap(), "voicecho.local:9000");
        for bad in ["https://", "a b:1", "h:0", "h:x", "user@h:1"] {
            assert!(host_port(bad).is_err(), "{bad}");
        }
        assert!(is_key("sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc="));
        assert!(!is_key("sha256//Xy96ZGBl7ULObmus5R49MQ"));
        assert!(!is_key("Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc="));
    }

    /// Plugin `voice`, installed, with `https://voicecho.local` listed.
    fn voice_listing_voicecho(root: &Path) {
        let mk = root.join("market");
        let d = mk.join("plugins/voice");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("echo-plugin.json"), json!({"id": "voice", "name": "Voice", "version": "0.1.0", "uses": {"network": {"hosts_from": ["at"]}}, "settings": [{"key": "at", "label": "Server", "type": "string"}]}).to_string()).unwrap();
        fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "voice", "source": "./plugins/voice"}]}).to_string()).unwrap();
        let name = market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        refresh(&name).unwrap();
        install("voice", Some(&name), &review("voice", Some(&name)).unwrap().uses).unwrap();
        set_setting("voice", "at", &json!("https://voicecho.local")).unwrap();
    }

    #[test]
    fn trusting_without_a_key_takes_the_one_echo_met_and_is_refused_once_it_met_two() {
        let (_env, root) = paths::testenv::folders("plugin-pins-two-keys");
        voice_listing_voicecho(&root);
        const A: &str = "sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=";
        const B: &str = "sha256//FnAJ8/qwoiAdJV/I5uE+TYf9kpG5WbHcCqDlA/p972I=";
        // One key met (again and again): trusted without naming it.
        saw("voice", "voicecho.local:443", A).unwrap();
        saw("voice", "voicecho.local:443", A).unwrap();
        assert_eq!(trust("voice", "voicecho.local", None).unwrap(), A);
        // Then B and A met there: which one the user looked at is unknown — they name it.
        saw("voice", "voicecho.local:443", B).unwrap();
        saw("voice", "voicecho.local:443", A).unwrap();
        let e = trust("voice", "voicecho.local", None).unwrap_err().to_string();
        assert!(e.contains("more than one key") && e.contains("--key"), "{e}");
        assert_eq!(pinned("voice", "voicecho.local:443").as_deref(), Some(A));
        assert_eq!(trust("voice", "voicecho.local", Some(B)).unwrap(), B);
        assert_eq!(pinned("voice", "voicecho.local:443").as_deref(), Some(B));
        // Forgotten, then one key met: without a key again.
        forget("voice", "voicecho.local").unwrap();
        saw("voice", "voicecho.local:443", A).unwrap();
        assert_eq!(trust("voice", "voicecho.local", None).unwrap(), A);
    }

    #[test]
    fn the_window_trusts_only_the_key_echo_met_and_only_on_the_users_yes() {
        let (_env, root) = paths::testenv::folders("plugin-pins-window");
        voice_listing_voicecho(&root);
        const MET: &str = "sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=";
        const OTHER: &str = "sha256//FnAJ8/qwoiAdJV/I5uE+TYf9kpG5WbHcCqDlA/p972I=";
        saw("voice", "voicecho.local:443", MET).unwrap();
        let op = |key: &str| page_op(&json!({"op": "cert_trust", "id": "voice", "address": "voicecho.local", "key": key}));
        // A key Echo didn't meet there: refused at once, no dialog.
        assert!(op(OTHER).unwrap_err().to_string().contains("met another certificate"));
        // The user says no: nothing pinned, and for a moment the page can't ask again.
        std::env::set_var("ECHOCTL_ALLOW_PINS", "no");
        op(MET).unwrap();
        paths::testenv::eventually("the answer taken", || !BUSY.lock().unwrap().contains_key("plugin:voice"));
        assert_eq!(pinned("voice", "voicecho.local:443"), None);
        assert!(op(MET).unwrap_err().to_string().contains("you said no a moment ago"));
        REFUSED.lock().unwrap().remove("voice");
        // Yes: pinned, and no longer met.
        std::env::set_var("ECHOCTL_ALLOW_PINS", "yes");
        op(MET).unwrap();
        paths::testenv::eventually("pinned", || pinned("voice", "voicecho.local:443").as_deref() == Some(MET));
        std::env::remove_var("ECHOCTL_ALLOW_PINS");
        assert_eq!(view("voice").unwrap(), vec![json!({"address": "voicecho.local:443", "pinned": MET, "seen": null, "changed": false})]);
        // Forgetting needs no dialog.
        page_op(&json!({"op": "cert_forget", "id": "voice", "address": "voicecho.local:443"})).unwrap();
        assert!(view("voice").unwrap().is_empty());
    }

    #[test]
    fn a_plugin_id_that_isnt_one_names_no_file_and_writes_nothing() {
        let (_env, _root) = paths::testenv::folders("plugin-pins-ids");
        let ai = paths::config_dir().join("ai.json");
        fs::create_dir_all(ai.parent().unwrap()).unwrap();
        fs::write(&ai, "{\"profiles\": []}").unwrap();
        const KEY: &str = "sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=";
        for id in ["../../ai", "../ai", "..", "a/b", "a\\b", "", "Voice", "voice.json"] {
            let forget_op = page_op(&json!({"op": "cert_forget", "id": id, "address": "voicecho.local:443"}));
            assert!(forget_op.unwrap_err().to_string().contains("isn't a plugin"), "{id}");
            assert!(page_op(&json!({"op": "cert_trust", "id": id, "address": "voicecho.local:443", "key": KEY})).is_err(), "{id}");
            assert!(forget(id, "voicecho.local:443").is_err(), "{id}");
            assert!(trust(id, "voicecho.local:443", Some(KEY)).is_err(), "{id}");
            assert!(saw(id, "voicecho.local:443", KEY).is_err(), "{id}");
            assert!(view(id).is_err(), "{id}");
            assert_eq!(pinned(id, "voicecho.local:443"), None, "{id}");
            remove_plugin(id);
        }
        assert_eq!(fs::read_to_string(&ai).unwrap(), "{\"profiles\": []}");
        assert!(!plugins_dir().join("pins").exists() || fs::read_dir(plugins_dir().join("pins")).unwrap().next().is_none());
    }
}
