//! A local AI profile's Ollama server: how much context it gives the profile's models (#224).
//! Ollama gives a model the context it was started with (`OLLAMA_CONTEXT_LENGTH`, else a default
//! from the machine's memory); Claude Code against it sets none, so a small one cuts a long call's
//! minutes without an error. `/api/ps` shows it for a loaded model. A server without `/api/ps`
//! (not Ollama) is left alone: no warning, and nothing but that one GET is sent to it.

use crate::ai::{self, Kind, Profile};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

/// Below this, a long call's transcript and Claude Code's own instructions no longer fit.
pub const FLOOR: u64 = 65_536;

/// The context each of `p`'s models has on its server (`None`: not loaded, or unknown), and a
/// warning for each under `FLOOR`. `load`: load a model that isn't (an empty prompt, as Ollama
/// documents) — only for an explicit check (`doctor`), never on a status read.
pub fn check(p: &Profile, load: bool) -> (Value, Vec<String>) {
    let (mut context, mut warnings) = (serde_json::Map::new(), vec![]);
    if p.kind != Kind::Local {
        return (json!({}), warnings);
    }
    let mut models = vec![p.live_model()];
    if let Some(m) = p.notes_model() {
        if !models.contains(&m) {
            models.insert(0, m);
        }
    }
    for m in models {
        let Some(n) = context_of(p, &m, load) else { continue };
        context.insert(m.clone(), json!(n));
        if n < FLOOR {
            warnings.push(format!(
                "{} gives {m} only {} tokens of context — long calls' minutes will miss parts. Set OLLAMA_CONTEXT_LENGTH={FLOOR} (or more) on that server and restart it",
                server(p),
                thousands(n)
            ));
        }
    }
    (Value::Object(context), warnings)
}

/// `echoctl doctor`: every local profile, its models loaded to check (`check` with `load`).
pub fn report() -> Vec<Value> {
    let profiles = ai::Settings::load().map(|s| s.all()).unwrap_or_default();
    profiles
        .iter()
        .filter(|p| p.kind == Kind::Local)
        .map(|p| {
            let (context, warnings) = check(p, true);
            json!({ "id": p.id, "name": p.name, "server": server(p), "context": context, "warnings": warnings })
        })
        .collect()
}

/// Where the server is, as the user would recognise it.
pub fn server(p: &Profile) -> String {
    let url = p.base_url.as_deref().unwrap_or("").trim_end_matches('/');
    if p.on_this_computer() && url.contains(":11434") {
        "Ollama on this computer".into()
    } else {
        url.into()
    }
}

/// 262144 → "262,144".
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn context_of(p: &Profile, model: &str, load: bool) -> Option<u64> {
    let ps = get_ps(p)?; // not Ollama, or not answering: nothing more is sent
    if let Some(n) = in_ps(&ps, model) {
        return Some(n);
    }
    if !load {
        return None;
    }
    post(p, "/api/generate", &json!({ "model": model, "prompt": "", "stream": false }), 600)?;
    in_ps(&get_ps(p)?, model)
}

/// `/api/ps` when the server is Ollama (a `models` list); else `None`.
fn get_ps(p: &Profile) -> Option<Value> {
    let v = request(p, "GET", "/api/ps", None, 5)?;
    v["models"].is_array().then_some(v)
}

/// A loaded model's context: by its name, or `<name>:latest` for a name without a tag.
fn in_ps(ps: &Value, model: &str) -> Option<u64> {
    let latest = format!("{model}:latest");
    ps["models"].as_array()?.iter().find(|m| [&m["name"], &m["model"]].iter().any(|n| n.as_str().is_some_and(|n| n == model || n == latest)))?["context_length"].as_u64()
}

fn post(p: &Profile, path: &str, body: &Value, secs: u64) -> Option<Value> {
    request(p, "POST", path, Some(body), secs)
}

/// One request through curl (as the live notes' requests: the config on stdin, so the token is
/// never on a command line; `-q` first, so `~/.curlrc` isn't read; no redirects). A 2xx JSON
/// answer, else `None`.
fn request(p: &Profile, method: &str, path: &str, body: Option<&Value>, secs: u64) -> Option<Value> {
    let base = p.base_url.as_deref().unwrap_or("").trim_end_matches('/');
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return None;
    }
    let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"));
    let mut c = format!("url = {}\nrequest = {}\nmax-time = {secs}\nconnect-timeout = 3\nwrite-out = \"\\n%{{http_code}}\"\n", q(&format!("{base}{path}")), q(method));
    if let Some(t) = p.token.as_deref().filter(|t| !t.is_empty()) {
        c.push_str(&format!("header = {}\n", q(&format!("Authorization: Bearer {t}"))));
    }
    if let Some(b) = body {
        c.push_str(&format!("header = \"Content-Type: application/json\"\ndata-binary = {}\n", q(&b.to_string())));
    }
    if ai::loopback(base) {
        c.push_str("noproxy = \"*\"\n");
    }
    let mut cmd = Command::new("curl");
    #[cfg(windows)]
    {
        use crate::app::NoWindow;
        cmd = cmd.no_window();
    }
    let mut child = cmd.args(["-q", "-sS", "-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    child.stdin.take()?.write_all(c.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    let out = String::from_utf8_lossy(&out.stdout);
    let (body, status) = out.rsplit_once('\n')?;
    status.trim().starts_with('2').then(|| serde_json::from_str(body).ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::sync::{Arc, Mutex};

    /// A stand-in server on 127.0.0.1: `/api/ps` lists `small-model` with `ctx` once loaded (by
    /// `POST /api/generate`, or `loaded` from the start); `ollama` false answers 404 to everything.
    /// Every request's `METHOD PATH` goes into the returned log.
    fn fake(ctx: u64, loaded: bool, ollama: bool) -> (String, Arc<Mutex<Vec<String>>>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let log = Arc::new(Mutex::new(vec![]));
        let seen = log.clone();
        std::thread::spawn(move || {
            let mut loaded = loaded;
            for s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut first = String::new();
                r.read_line(&mut first).unwrap();
                let mut len = 0;
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                let line = first.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
                seen.lock().unwrap().push(line.clone());
                let (code, reply) = match (ollama, line.as_str()) {
                    (true, "GET /api/ps") => (200, json!({ "models": if loaded { json!([{ "name": "small-model:latest", "model": "small-model:latest", "context_length": ctx }]) } else { json!([]) } })),
                    (true, "POST /api/generate") => {
                        loaded = true;
                        (200, json!({ "done": true }))
                    }
                    _ => (404, json!({ "error": "not found" })),
                };
                let b = reply.to_string();
                let _ = (&s).write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len()).as_bytes());
            }
        });
        (url, log)
    }

    fn local(url: &str) -> Profile {
        Profile { id: "q".into(), name: "Q".into(), kind: Kind::Local, base_url: Some(url.into()), token: None, live_model: Some("small-model".into()), notes_model: Some("small-model".into()), account: None, email: None, sso: false }
    }

    #[test]
    fn a_small_context_is_warned_about_and_a_large_one_is_not() {
        let (url, _) = fake(4096, true, true);
        let (ctx, w) = check(&local(&url), false);
        assert_eq!(ctx, json!({ "small-model": 4096 }));
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains(&url) && w[0].contains("only 4,096 tokens") && w[0].contains("OLLAMA_CONTEXT_LENGTH=65536"), "{}", w[0]);
        let (url, _) = fake(262_144, true, true);
        assert_eq!(check(&local(&url), false), (json!({ "small-model": 262144 }), vec![]));
    }

    #[test]
    fn a_status_read_never_loads_a_model_and_doctor_does() {
        let (url, log) = fake(4096, false, true);
        assert_eq!(check(&local(&url), false), (json!({}), vec![]), "not loaded: unknown, no warning");
        assert_eq!(*log.lock().unwrap(), ["GET /api/ps"]);
        let (ctx, w) = check(&local(&url), true);
        assert_eq!((ctx, w.len()), (json!({ "small-model": 4096 }), 1));
        assert_eq!(*log.lock().unwrap(), ["GET /api/ps", "GET /api/ps", "POST /api/generate", "GET /api/ps"]);
    }

    #[test]
    fn a_server_that_isnt_ollama_gets_one_get_and_no_warning() {
        let (url, log) = fake(4096, false, false);
        assert_eq!(check(&local(&url), true), (json!({}), vec![]));
        assert_eq!(*log.lock().unwrap(), ["GET /api/ps"], "no load is sent to it");
        let mut down = local("http://127.0.0.1:9");
        assert_eq!(check(&down, true), (json!({}), vec![]), "not answering: no warning either");
        down.kind = Kind::Claude;
        assert_eq!(check(&down, true), (json!({}), vec![]));
    }

    #[test]
    fn the_server_is_named_as_the_user_knows_it() {
        assert_eq!(server(&local("http://localhost:11434/")), "Ollama on this computer");
        assert_eq!(server(&local("http://gpu-box.example:11434")), "http://gpu-box.example:11434");
        assert_eq!((thousands(4096), thousands(262_144), thousands(65_536), thousands(512)), ("4,096".into(), "262,144".into(), "65,536".into(), "512".into()));
    }
}
