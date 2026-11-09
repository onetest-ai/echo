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

/// What a check found on a profile's server.
#[derive(Debug, Default, PartialEq)]
pub struct Check {
    /// model → tokens of context, for each model it could see loaded
    pub context: serde_json::Map<String, Value>,
    /// a model under `FLOOR`
    pub warnings: Vec<String>,
    /// Ollama answered but a model couldn't be loaded (not pulled, out of memory…)
    pub problems: Vec<String>,
    /// loaded, but this Ollama's `/api/ps` has no `context_length` (an older version)
    pub unreported: Vec<String>,
}

/// The context each of `p`'s models has on its server, a warning for each under `FLOOR`. `load`:
/// load a model that isn't (an empty prompt, as Ollama documents; it stays loaded 30 s) — only for
/// an explicit check (`doctor`), never on a status read. `/api/ps` is read once, and again only
/// after a load.
pub fn check(p: &Profile, load: bool) -> Check {
    let mut c = Check::default();
    if p.kind != Kind::Local {
        return c;
    }
    let Some(mut ps) = get_ps(p) else { return c }; // not Ollama, or not answering: nothing more is sent
    let mut models = vec![p.live_model()];
    if let Some(m) = p.notes_model().filter(|m| !models.contains(m)) {
        models.insert(0, m);
    }
    for m in models {
        let mut n = in_ps(&ps, &m);
        if n.is_none() && load {
            match request(p, "POST", "/api/generate", Some(&json!({ "model": m, "prompt": "", "stream": false, "keep_alive": "30s" })), 120) {
                Ok(_) => {
                    ps = get_ps(p).unwrap_or(ps);
                    n = in_ps(&ps, &m);
                }
                Err(why) => c.problems.push(format!("{} has no model {m}, or couldn't load it: {why}", server(p))),
            }
        }
        let Some(n) = n else { continue };
        let Some(n) = n else {
            c.unreported.push(m);
            continue;
        };
        c.context.insert(m.clone(), json!(n));
        if n < FLOOR {
            c.warnings.push(format!(
                "{} gives {m} only {} tokens of context — long calls' minutes will miss parts. Set OLLAMA_CONTEXT_LENGTH={FLOOR} (or more) on that server and restart it",
                server(p),
                thousands(n)
            ));
        }
    }
    c
}

/// `ai status`'s entry for `p` with what its server shows now (`context`, `warnings`); read only.
pub fn with_context(p: &Profile, mut entry: Value) -> Value {
    let c = check(p, false);
    entry["context"] = Value::Object(c.context);
    entry["warnings"] = json!(c.warnings);
    entry
}

/// The local profiles, for `doctor`.
pub fn local_profiles() -> Vec<Profile> {
    ai::Settings::load().map(|s| s.all()).unwrap_or_default().into_iter().filter(|p| p.kind == Kind::Local).collect()
}

/// `doctor`'s entry for `p`: its models loaded to check (`check` with `load`).
pub fn report(p: &Profile) -> Value {
    let c = check(p, true);
    json!({ "id": p.id, "name": p.name, "server": server(p), "context": c.context, "warnings": c.warnings, "problems": c.problems, "unreported": c.unreported })
}

/// `doctor`'s lines for one `report`.
pub fn print(r: &Value) {
    for l in lines(r) {
        println!("{l}");
    }
}

fn lines(r: &Value) -> Vec<String> {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut ctx: Vec<String> = r["context"].as_object().into_iter().flatten().map(|(m, n)| format!("{m}: {} tokens of context", thousands(n.as_u64().unwrap_or(0)))).collect();
    ctx.extend(r["unreported"].as_array().into_iter().flatten().map(|m| format!("{}: context not reported by this Ollama (a newer one shows it)", s(m))));
    let notes: Vec<String> = ["warnings", "problems"].iter().flat_map(|k| r[k].as_array().into_iter().flatten().map(s)).collect();
    // not `?` for unknown: doctor's last line asks to fix the ✗/? items
    let mark = if !notes.is_empty() { "⚠" } else if ctx.is_empty() { "–" } else { "✓" };
    let what = match (ctx.is_empty(), notes.is_empty()) {
        (false, _) => ctx.join(", "),
        (true, true) => "context unknown (not an Ollama server, or not answering)".into(),
        (true, false) => "context unknown".into(), // Ollama answered: the lines below say why
    };
    let mut out = vec![format!("{mark} {} — {what}", s(&r["name"]))];
    out.extend(notes.into_iter().map(|n| format!("    {n}")));
    out
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

/// `/api/ps` when the server is Ollama (a `models` list); else `None`.
fn get_ps(p: &Profile) -> Option<Value> {
    request(p, "GET", "/api/ps", None, 5).ok().filter(|v| v["models"].is_array())
}

/// A model's name as Ollama lists it: no registry or `library/` prefix, `:latest` when it has no
/// tag, case ignored.
fn canonical(name: &str) -> String {
    let n = name.trim().to_lowercase();
    let n = n.strip_prefix("registry.ollama.ai/").unwrap_or(&n);
    let n = n.strip_prefix("library/").unwrap_or(n);
    if n.rsplit('/').next().unwrap_or(n).contains(':') {
        n.to_string()
    } else {
        format!("{n}:latest")
    }
}

/// A loaded model's context, by its name (`canonical` on both sides): `None` when it isn't loaded,
/// `Some(None)` when it is but this Ollama doesn't report `context_length`.
fn in_ps(ps: &Value, model: &str) -> Option<Option<u64>> {
    let want = canonical(model);
    Some(ps["models"].as_array()?.iter().find(|m| [&m["name"], &m["model"]].iter().any(|n| n.as_str().is_some_and(|n| canonical(n) == want)))?["context_length"].as_u64())
}

/// One request through curl (as the live notes' requests: the config on stdin, so the token is
/// never on a command line; `-q` first, so `~/.curlrc` isn't read; no redirects). A 2xx JSON
/// answer, else why not.
fn request(p: &Profile, method: &str, path: &str, body: Option<&Value>, secs: u64) -> Result<Value, String> {
    let base = p.base_url.as_deref().unwrap_or("").trim_end_matches('/');
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err("no http(s) address".into());
    }
    let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"));
    let mut c = format!("url = {}\nrequest = {}\nmax-time = {secs}\nconnect-timeout = 3\nwrite-out = \"\\n%{{http_code}}\"\n", q(&format!("{base}{path}")), q(method));
    if let Some(t) = p.token.as_deref().filter(|t| !t.is_empty()) {
        // both, as a proxy in front of Ollama may want either (the live notes send `x-api-key`)
        c.push_str(&format!("header = {}\nheader = {}\n", q(&format!("Authorization: Bearer {t}")), q(&format!("x-api-key: {t}"))));
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
    let mut child = cmd.args(["-q", "-sS", "-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("curl: {e}"))?;
    child.stdin.take().ok_or("curl: no stdin")?.write_all(c.as_bytes()).map_err(|e| format!("curl: {e}"))?;
    let out = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (body, status) = text.rsplit_once('\n').unwrap_or(("", &text));
    let status = status.trim();
    if !out.status.success() || status == "000" {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("no answer").to_string());
    }
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if !status.starts_with('2') {
        let why = v["error"].as_str().map(str::to_string).unwrap_or_else(|| body.chars().take(200).collect());
        return Err(format!("HTTP {status}: {why}"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::sync::{Arc, Mutex};

    /// A stand-in server on 127.0.0.1: `/api/ps` lists `small-model` with `ctx` once loaded (by
    /// `POST /api/generate`, or `loaded` from the start); a load of another model is a 404 (not
    /// pulled); `ollama` false answers 404 to everything. Every request's `METHOD PATH` (and a
    /// POST's body) goes into the returned log.
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
                seen.lock().unwrap().push(if body.is_empty() { line.clone() } else { format!("{line} {}", String::from_utf8_lossy(&body)) });
                let asked: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let (code, reply) = match (ollama, line.as_str()) {
                    (true, "GET /api/ps") => (200, json!({ "models": if loaded { json!([{ "name": "small-model:latest", "model": "small-model:latest", "context_length": ctx }]) } else { json!([]) } })),
                    (true, "POST /api/generate") if asked["model"] == "small-model" => {
                        loaded = true;
                        (200, json!({ "done": true }))
                    }
                    (true, "POST /api/generate") => (404, json!({ "error": format!("model '{}' not found", asked["model"].as_str().unwrap_or("")) })),
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

    fn requests(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().iter().map(|l| l.split(' ').take(2).collect::<Vec<_>>().join(" ")).collect()
    }

    #[test]
    fn a_small_context_is_warned_about_and_a_large_one_is_not() {
        let (url, _) = fake(4096, true, true);
        let c = check(&local(&url), false);
        assert_eq!(Value::Object(c.context), json!({ "small-model": 4096 }));
        assert_eq!(c.warnings.len(), 1, "{:?}", c.warnings);
        assert!(c.warnings[0].contains(&url) && c.warnings[0].contains("only 4,096 tokens") && c.warnings[0].contains("OLLAMA_CONTEXT_LENGTH=65536"), "{}", c.warnings[0]);
        let (url, _) = fake(262_144, true, true);
        let c = check(&local(&url), false);
        assert_eq!((Value::Object(c.context), c.warnings.len()), (json!({ "small-model": 262144 }), 0));
    }

    #[test]
    fn a_status_read_never_loads_a_model_and_doctor_does_briefly() {
        let (url, log) = fake(4096, false, true);
        assert_eq!(check(&local(&url), false), Check::default(), "not loaded: unknown, no warning");
        assert_eq!(requests(&log), ["GET /api/ps"]);
        let c = check(&local(&url), true);
        assert_eq!((Value::Object(c.context), c.warnings.len()), (json!({ "small-model": 4096 }), 1));
        assert_eq!(requests(&log), ["GET /api/ps", "GET /api/ps", "POST /api/generate", "GET /api/ps"], "one read, the load, one read after it");
        let load = log.lock().unwrap()[2].clone();
        assert!(load.contains("\"keep_alive\":\"30s\"") && load.contains("\"prompt\":\"\""), "{load}");
    }

    #[test]
    fn a_model_that_cant_be_loaded_is_said_so_not_that_the_server_isnt_ollama() {
        let (url, _) = fake(4096, false, true);
        let mut p = local(&url);
        p.notes_model = Some("not-pulled".into());
        let c = check(&p, true);
        assert_eq!(c.problems.len(), 1, "{c:?}");
        assert!(c.problems[0].contains("has no model not-pulled, or couldn't load it") && c.problems[0].contains("HTTP 404"), "{}", c.problems[0]);
        assert_eq!(Value::Object(c.context), json!({ "small-model": 4096 }), "the other model is still checked");
    }

    #[test]
    fn a_server_that_isnt_ollama_gets_one_get_and_no_warning() {
        let (url, log) = fake(4096, false, false);
        assert_eq!(check(&local(&url), true), Check::default());
        assert_eq!(requests(&log), ["GET /api/ps"], "no load is sent to it");
        let mut down = local("http://127.0.0.1:9");
        assert_eq!(check(&down, true), Check::default(), "not answering: no warning either");
        down.kind = Kind::Claude;
        assert_eq!(check(&down, true), Check::default());
    }

    #[test]
    fn model_names_match_as_ollama_lists_them() {
        let ps = json!({ "models": [{ "name": "qwen3.8:27b-mlx", "context_length": 8192 }, { "name": "llama3.2:latest", "context_length": 4096 }] });
        assert_eq!(in_ps(&ps, "Qwen3.8:27B-MLX"), Some(Some(8192)));
        assert_eq!(in_ps(&ps, "library/qwen3.8:27b-mlx"), Some(Some(8192)));
        assert_eq!(in_ps(&ps, "registry.ollama.ai/library/llama3.2"), Some(Some(4096)));
        assert_eq!(in_ps(&ps, "llama3.2"), Some(Some(4096)));
        assert_eq!(in_ps(&ps, "qwen3.8"), None, "another tag is another model");
    }

    #[test]
    fn an_older_ollama_without_context_length_is_said_so_and_not_reloaded() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let hits = Arc::new(Mutex::new(0));
        let seen = hits.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut first = String::new();
                BufReader::new(s.try_clone().unwrap()).read_line(&mut first).unwrap();
                *seen.lock().unwrap() += 1;
                let b = json!({ "models": [{ "name": "small-model:latest", "model": "small-model:latest" }] }).to_string();
                let _ = (&s).write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len()).as_bytes());
            }
        });
        let c = check(&local(&url), true);
        assert_eq!((c.unreported, c.warnings.len(), c.problems.len(), c.context.len()), (vec!["small-model".to_string()], 0, 0, 0));
        assert_eq!(*hits.lock().unwrap(), 1, "listed: no load sent");
        assert_eq!(lines(&report(&local(&url))), ["✓ Q — small-model: context not reported by this Ollama (a newer one shows it)"], "not \"not an Ollama server\"");
        let none = json!({ "name": "Other", "context": {}, "warnings": [], "problems": [], "unreported": [] });
        assert_eq!(lines(&none), ["– Other — context unknown (not an Ollama server, or not answering)"]);
    }

    #[test]
    fn the_server_is_named_as_the_user_knows_it() {
        assert_eq!(server(&local("http://localhost:11434/")), "Ollama on this computer");
        assert_eq!(server(&local("http://gpu-box.example:11434")), "http://gpu-box.example:11434");
        assert_eq!((thousands(4096), thousands(262_144), thousands(65_536), thousands(512)), ("4,096".into(), "262,144".into(), "65,536".into(), "512".into()));
    }
}
