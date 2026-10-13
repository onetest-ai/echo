//! Transcription on another computer: a laptop without a usable GPU sends each utterance to a
//! transcription server and gets the text back, instead of decoding on its own CPU.
//!
//! - Client (`Client`): a server URL (`http://` or `https://`) and an API key, set with
//!   Settings → Connect to a transcription server… or `echoctl server use <url> --key <key>`,
//!   kept in `<config dir>/transcription-server.json`. Only utterances go out (16 kHz mono,
//!   ≤ 25 s each); voice detection, the filters, echo cancellation and the transcript stay on
//!   the laptop. While the server can't be reached this computer decodes (`transcribe.rs`).
//! - Server (`run_in_app`): any Echo with Settings → Transcribe for other computers on (or
//!   `echoctl server share on`) decodes for computers that have its URL and key, with its own
//!   model on its own GPU. Plain HTTP on the local network (put a TLS proxy in front for
//!   https); it keeps no audio and no text and logs no content.
//!
//! Protocol (ours, versioned by the path):
//! - `GET /v1/hello` → `{name, model, engine, version}`
//! - `POST /v1/utterance`: a little-endian u32 length, that many bytes of JSON
//!   (`{languages, prev, prompt}`), then the samples as 16-bit little-endian PCM →
//!   `{lang_id, lang, segs: [[start, end, text]]}`.
//!
//! Both need `Authorization: Bearer <key>`; a wrong key gets 401 after a 1 s pause.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::Config;
use crate::transcribe::{Decoded, Engine, EngineState, Piece};
use crate::{models, paths};

const CLIENT_FILE: &str = "transcription-server.json";
const SERVE_FILE: &str = "transcription-serve.json";
pub const DEFAULT_PORT: u16 = 47800;
/// 25 s of 16-bit audio is 800 KB; the rest is the JSON part.
const MAX_BODY: usize = 4 << 20;
const MAX_HEADERS: usize = 16 << 10;
const MAX_CONNECTIONS: usize = 8;
/// The model is unloaded after this long without a request (it holds GPU memory).
const IDLE_UNLOAD: Duration = Duration::from_secs(10 * 60);
/// A queued utterance can wait behind others' on the server.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

// ── client ──────────────────────────────────────────────────────────────────────────────────

/// The server refused the request itself (wrong key, bad request): retrying won't help.
#[derive(Debug)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

#[derive(Serialize, Deserialize, Clone)]
struct ClientSettings {
    url: String,
    key: String,
    /// The server's name when it was connected (for menus).
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub name: String,
    pub model: String,
    pub engine: String,
    pub version: String,
}

pub struct Client {
    url: String,
    key: String,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(url: &str, key: &str) -> Result<Client> {
        let url = url.trim().trim_end_matches('/').to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("the server URL must start with http:// or https:// (e.g. http://192.168.1.20:{DEFAULT_PORT})");
        }
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
            .build()
            .into();
        Ok(Client { url, key: key.trim().to_string(), agent })
    }

    /// The server this computer uses, if one is set.
    pub fn configured() -> Option<Client> {
        let s = load_client()?;
        Client::new(&s.url, &s.key).ok()
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    fn check(status: u16, body: &str) -> Result<()> {
        match status {
            200 => Ok(()),
            401 | 403 => Err(Refused("the server rejected the API key".into()).into()),
            400 | 404 | 405 | 413 => Err(Refused(format!("the server refused the request ({status}): {}", body.trim())).into()),
            _ => bail!("the server answered {status}: {}", body.trim()),
        }
    }

    pub fn hello(&self) -> Result<Hello> {
        let mut r = self
            .agent
            .get(format!("{}/v1/hello", self.url))
            .header("Authorization", format!("Bearer {}", self.key))
            .config()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .call()
            .with_context(|| format!("could not reach {}", self.url))?;
        let body = r.body_mut().read_to_string()?;
        Self::check(r.status().as_u16(), &body)?;
        serde_json::from_str(&body).context("not an Echo transcription server")
    }

    /// Decode one utterance on the server (see `Engine::utterance`).
    pub fn utterance(&self, chunk: &[f32], prev: Option<i32>, allowed: &[i32], prompt: Option<&str>) -> Result<(Option<i32>, String, Decoded)> {
        let languages: Vec<&str> = allowed.iter().filter_map(|l| whisper_rs::get_lang_str(*l)).collect();
        let head = serde_json::to_vec(&json!({
            "languages": languages,
            "prev": prev.and_then(whisper_rs::get_lang_str),
            "prompt": prompt,
        }))?;
        let mut body = Vec::with_capacity(4 + head.len() + chunk.len() * 2);
        body.extend((head.len() as u32).to_le_bytes());
        body.extend(&head);
        for s in chunk {
            body.extend(((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
        }
        let mut r = self
            .agent
            .post(format!("{}/v1/utterance", self.url))
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/octet-stream")
            .send(&body[..])
            .with_context(|| format!("could not reach {}", self.url))?;
        let text = r.body_mut().read_to_string()?;
        Self::check(r.status().as_u16(), &text)?;
        let v: Answer = serde_json::from_str(&text).context("unexpected answer from the server")?;
        let mut words = v.words.into_iter();
        let segs = v.segs.into_iter().map(|(a, b, t)| Piece::new(a, b, t, words.next().unwrap_or_default())).collect();
        Ok((v.lang_id.as_deref().and_then(whisper_rs::get_lang_id), v.lang, segs))
    }
}

/// The server's answer. `words` (per sentence, its words' times) came later: older servers
/// leave it out, older clients ignore it.
#[derive(Serialize, Deserialize)]
struct Answer {
    lang_id: Option<String>,
    lang: String,
    segs: Vec<(f64, f64, String)>,
    #[serde(default)]
    words: Vec<Vec<[f64; 2]>>,
}

fn load_client() -> Option<ClientSettings> {
    serde_json::from_slice(&std::fs::read(paths::config_dir().join(CLIENT_FILE)).ok()?).ok()
}

/// Write a settings file only this user can read (it holds a key).
fn write_private(name: &str, bytes: &[u8]) -> Result<()> {
    let dir = paths::config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    let tmp = dir.join(format!("{name}.tmp"));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Connect to a server: check the URL and key, then keep them. Returns what the server said.
pub fn connect(url: &str, key: &str) -> Result<Hello> {
    let c = Client::new(url, key)?;
    let hello = c.hello()?;
    let s = ClientSettings { url: c.url.clone(), key: c.key.clone(), name: Some(hello.name.clone()) };
    write_private(CLIENT_FILE, &serde_json::to_vec_pretty(&s)?)?;
    Ok(hello)
}

/// Stop using the server (transcribe on this computer again).
pub fn disconnect() -> Result<()> {
    match std::fs::remove_file(paths::config_dir().join(CLIENT_FILE)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// The server in use, for menus: "<name> (<url>)". Never touches the network.
pub fn client_label() -> Option<String> {
    let s = load_client()?;
    Some(match s.name {
        Some(n) => format!("{n} ({})", s.url),
        None => s.url,
    })
}

/// `echoctl server status`.
pub fn print_status() -> Result<()> {
    match Client::configured() {
        Some(c) => match c.hello() {
            Ok(h) => println!("transcribing on {} — {} ({}, {}, Echo {})", c.url, h.name, h.model, h.engine, h.version),
            Err(e) => println!("transcribing on {} — not reachable now ({e:#}); this computer decodes meanwhile", c.url),
        },
        None => println!("transcribing on this computer"),
    }
    let s = serve_settings();
    if s.on {
        let state = if crate::watch::running_watcher().is_some() { "" } else { " (when Echo runs)" };
        println!("serving other computers{state}: URL {}  API key {}", serve_url(&s), s.key);
    } else {
        println!("not serving other computers (`echoctl server share on`)");
    }
    Ok(())
}

// ── server ──────────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ServeSettings {
    pub on: bool,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub key: String,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

pub fn serve_settings() -> ServeSettings {
    std::fs::read(paths::config_dir().join(SERVE_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(ServeSettings { on: false, port: DEFAULT_PORT, key: String::new() })
}

/// Turn serving on or off (the running app picks it up within 2 s). A key is made the first
/// time; `new_key` replaces it (computers using the old one must connect again).
pub fn set_serving(on: bool, new_key: bool) -> Result<ServeSettings> {
    let mut s = serve_settings();
    s.on = on;
    if s.key.is_empty() || new_key {
        s.key = make_key()?;
    }
    write_private(SERVE_FILE, &serde_json::to_vec_pretty(&s)?)?;
    Ok(s)
}

/// 16 letters and digits without look-alikes, in groups of 4: easy to read out and type.
fn make_key() -> Result<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw).map_err(|e| anyhow!("no randomness: {e}"))?;
    let chars: Vec<char> = raw.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect();
    Ok(chars.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-"))
}

/// Keys compare without case, dashes or spaces (read out, typed by hand).
fn normalize_key(k: &str) -> String {
    k.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

fn key_matches(given: &str, key: &str) -> bool {
    let (a, b) = (normalize_key(given), normalize_key(key));
    !b.is_empty() && a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// This computer's name, for the client's menus.
fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output() {
        let n = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !n.is_empty() {
            return n;
        }
    }
    std::env::var("COMPUTERNAME").ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "Echo".into())
}

/// The address other computers on the network reach this one at (the interface the default
/// route uses; a UDP connect sends nothing).
fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// The URL to give other computers.
pub fn serve_url(s: &ServeSettings) -> String {
    format!("http://{}:{}", lan_ip().unwrap_or_else(|| "<this computer's address>".into()), s.port)
}

struct Loaded {
    engine: Engine,
    state: EngineState,
    model: String,
    used: Instant,
}

/// The server's model, loaded at the first request; one utterance at a time.
static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);
/// Wrong keys are answered one at a time, each after a pause.
static AUTH_FAIL: Mutex<()> = Mutex::new(());
static CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

/// The app's server thread: follows `transcription-serve.json` (checked every 2 s), listening
/// while it says on. Never returns.
pub fn run_in_app() {
    let mut current: Option<(ServeSettings, TcpListener)> = None;
    loop {
        let want = serve_settings();
        let same = current.as_ref().is_some_and(|(s, _)| *s == want);
        if !same {
            if current.take().is_some() {
                eprintln!("transcription server: stopped");
            }
            if want.on && !want.key.is_empty() {
                match bind(want.port) {
                    Ok(l) => {
                        eprintln!("transcription server: listening at {}", serve_url(&want));
                        current = Some((want.clone(), l));
                    }
                    Err(e) => eprintln!("transcription server: {e:#}"),
                }
            }
        }
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            match &current {
                Some((s, l)) => accept_ready(l, &s.key),
                None => std::thread::sleep(Duration::from_millis(250)),
            }
        }
        unload_if_idle();
        // Not serving and the settings file is gone or off: nothing else to do but keep looking.
        if current.is_none() && !want.on {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

/// `echoctl server run`: serve in the foreground (Ctrl-C stops), whatever the app does.
pub fn run_foreground(port: Option<u16>) -> Result<()> {
    let mut s = serve_settings();
    if s.key.is_empty() {
        s = set_serving(s.on, true)?;
    }
    let port = port.unwrap_or(s.port);
    let l = bind(port)?;
    let s = ServeSettings { port, ..s };
    println!("transcription server at {}  key {}", serve_url(&s), s.key);
    loop {
        accept_ready(&l, &s.key);
        unload_if_idle();
    }
}

fn bind(port: u16) -> Result<TcpListener> {
    let l = TcpListener::bind(("0.0.0.0", port)).with_context(|| format!("port {port} is in use"))?;
    l.set_nonblocking(true)?;
    Ok(l)
}

/// Accept what is waiting (each connection on its own thread), else sleep a moment.
fn accept_ready(l: &TcpListener, key: &str) {
    match l.accept() {
        Ok((stream, _)) => {
            if CONNECTIONS.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
                let _ = respond(&stream, 503, "busy");
                return;
            }
            let key = key.to_string();
            std::thread::spawn(move || {
                if let Err(e) = handle(&stream, &key) {
                    let _ = respond(&stream, 500, &format!("{e:#}"));
                }
                CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
            });
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
        Err(_) => std::thread::sleep(Duration::from_millis(250)),
    }
}

fn unload_if_idle() {
    if let Ok(mut g) = LOADED.try_lock() {
        if g.as_ref().is_some_and(|l| l.used.elapsed() > IDLE_UNLOAD) {
            *g = None;
            eprintln!("transcription server: model unloaded (idle)");
        }
    }
}

fn respond(mut s: &TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let ctype = if body.starts_with('{') { "application/json" } else { "text/plain; charset=utf-8" };
    write!(s, "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())?;
    s.flush()?;
    Ok(())
}

fn handle(stream: &TcpStream, key: &str) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    r.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let (mut auth, mut length, mut seen) = (String::new(), 0usize, line.len());
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 {
            break;
        }
        seen += h.len();
        if seen > MAX_HEADERS {
            return respond(stream, 400, "headers too large");
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((name, value)) = h.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "authorization" => auth = value.trim().strip_prefix("Bearer ").unwrap_or("").to_string(),
                "content-length" => length = value.trim().parse().unwrap_or(usize::MAX),
                _ => {}
            }
        }
    }
    if !key_matches(&auth, key) {
        let _g = AUTH_FAIL.lock().unwrap_or_else(|e| e.into_inner());
        std::thread::sleep(Duration::from_secs(1));
        return respond(stream, 401, "wrong API key");
    }
    match (method.as_str(), path.as_str()) {
        ("GET", "/v1/hello") => {
            let model = Config::load().ok().and_then(|c| c.model).unwrap_or_default();
            let hello = Hello { name: computer_name(), model, engine: crate::gpu::engine_status(), version: env!("CARGO_PKG_VERSION").into() };
            respond(stream, 200, &serde_json::to_string(&hello)?)
        }
        ("POST", "/v1/utterance") => {
            if length > MAX_BODY {
                return respond(stream, 413, "utterance too large");
            }
            let mut body = vec![0u8; length];
            r.read_exact(&mut body)?;
            match decode_request(&body) {
                Ok(answer) => respond(stream, 200, &serde_json::to_string(&answer)?),
                Err(e) if e.downcast_ref::<Refused>().is_some() => respond(stream, 400, &format!("{e:#}")),
                Err(e) => respond(stream, 503, &format!("{e:#}")),
            }
        }
        _ => respond(stream, 404, "not found"),
    }
}

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    languages: Vec<String>,
    prev: Option<String>,
    prompt: Option<String>,
}

/// Split the body into its JSON part and samples.
fn parse_body(body: &[u8]) -> Result<(Request, Vec<f32>)> {
    let bad = |m: &str| anyhow::Error::new(Refused(m.into()));
    let n = body.get(..4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(|| bad("empty request"))?;
    let head = body.get(4..4 + n).ok_or_else(|| bad("truncated request"))?;
    let req: Request = serde_json::from_slice(head).map_err(|e| bad(&format!("bad request: {e}")))?;
    let pcm = &body[4 + n..];
    if !pcm.len().is_multiple_of(2) {
        return Err(bad("odd number of sample bytes"));
    }
    Ok((req, pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32).collect()))
}

fn decode_request(body: &[u8]) -> Result<Answer> {
    let (req, audio) = parse_body(body)?;
    let lang = |l: &str| whisper_rs::get_lang_id(l).ok_or_else(|| anyhow::Error::new(Refused(format!("unknown language '{l}'"))));
    let allowed = req.languages.iter().map(|l| lang(l)).collect::<Result<Vec<_>>>()?;
    let prev = req.prev.as_deref().map(lang).transpose()?;
    let mut g = LOADED.lock().unwrap_or_else(|e| e.into_inner());
    let model = Config::load().ok().and_then(|c| c.model).unwrap_or_default();
    if g.as_ref().is_some_and(|l| l.model != model) {
        *g = None; // the active model was changed here
    }
    if g.is_none() {
        let path = models::active_model_path()?;
        if !path.exists() {
            bail!("the server has no speech model downloaded");
        }
        eprintln!("transcription server: loading {model}");
        let engine = Engine::load(&path, crate::transcribe::final_threads())?;
        let state = engine.state()?;
        *g = Some(Loaded { engine, state, model, used: Instant::now() });
    }
    let l = g.as_mut().expect("just loaded");
    l.used = Instant::now();
    let (lang_id, lang, segs) = l.engine.utterance(&mut l.state, prev, &allowed, req.prompt.as_deref(), &audio)?;
    let words = segs.iter().map(|p| p.words.clone()).collect();
    let segs = segs.into_iter().map(|p| (p.start, p.end, p.text)).collect();
    Ok(Answer { lang_id: lang_id.and_then(whisper_rs::get_lang_str).map(str::to_owned), lang, segs, words })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_however_they_are_typed() {
        let k = make_key().unwrap();
        assert_eq!(k.len(), 19);
        assert!(key_matches(&k, &k));
        assert!(key_matches(&k.to_lowercase().replace('-', " "), &k));
        assert!(!key_matches("", &k));
        assert!(!key_matches("ABCD-EFGH-JKMN-PQRT", &k) || k == "ABCD-EFGH-JKMN-PQRT");
        assert!(!key_matches("", ""));
    }

    #[test]
    fn request_body_round_trips() {
        let head = br#"{"languages":["en","uk"],"prev":"uk","prompt":"Echo"}"#;
        let mut body = (head.len() as u32).to_le_bytes().to_vec();
        body.extend(head);
        body.extend(i16::MAX.to_le_bytes());
        body.extend(0i16.to_le_bytes());
        let (req, audio) = parse_body(&body).unwrap();
        assert_eq!(req.languages, ["en", "uk"]);
        assert_eq!(req.prev.as_deref(), Some("uk"));
        assert_eq!(audio, [1.0, 0.0]);
        assert!(parse_body(&body[..3]).is_err());
        assert!(parse_body(&body[..body.len() - 1]).is_err());
    }
}
