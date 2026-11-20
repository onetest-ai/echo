//! Traces of what Echo runs (#859), kept on this computer for `KEEP_DAYS`: each background job's run and each Claude
//! Code print run Echo starts is a trace — Echo's own spans and events, the run's arguments, input and every stream
//! event in full, and Claude Code's own spans and events (its OpenTelemetry, taken by `receiver`) nested under them
//! through `TRACEPARENT`.
//!
//! Records go on a bounded queue that a background thread appends to `<config dir>/telemetry/<YYYY-MM-DD>/<trace>.jsonl`,
//! one JSON line each, under that day's lock: sending never waits (a full queue drops the record and counts it). A
//! trace that ends (its root span) is also one line of that day's `index.jsonl`, which `list` reads. Day folders older
//! than `KEEP_DAYS` are removed. A line: `{"t": "span" | "event", "source": "echo" | "claude-code", "trace", "span",
//! "parent", "name", "start", "end" | "at", "attrs"}`.
//!
//! Claude Code reports to Echo only when nothing else configures its telemetry (`configured_elsewhere`; after the login
//! shell's start-up files, `ai::shell_script` checks again): whatever already applies to a run is left as it is. Its
//! own logging of prompts and replies stays off: the text comes from the stream Echo reads.

use crate::ai::Profile;
use crate::paths;
use chrono::{DateTime, Local, NaiveDate, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub mod export;
mod receiver;

const DIR: &str = "telemetry";
const INDEX: &str = "index.jsonl";
/// Detail is kept this many days, today included.
pub const KEEP_DAYS: i64 = 7;
/// A run's numbers (its line of the day's index) are kept this many days.
pub const NUMBERS_DAYS: i64 = 90;
const QUEUE: usize = 20_000;

// ── the queue and the store ──────────────────────────────────────────────────────────────────

enum Msg {
    Line { dir: PathBuf, day: String, file: String, line: Value },
    Flush(SyncSender<()>),
}

static TX: OnceLock<SyncSender<Msg>> = OnceLock::new();
static DROPPED: AtomicU64 = AtomicU64::new(0);

fn tx() -> &'static SyncSender<Msg> {
    TX.get_or_init(|| {
        let (tx, rx) = sync_channel(QUEUE);
        let _ = std::thread::Builder::new().name("telemetry".into()).spawn(move || write_loop(rx));
        tx
    })
}

/// Queue a line for `file` of today's folder; never waits: a full queue drops it (`dropped`).
fn queue(file: String, line: Value) {
    let msg = Msg::Line { dir: paths::config_dir().join(DIR), day: Local::now().format("%Y-%m-%d").to_string(), file, line };
    if tx().try_send(msg).is_err() {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

/// A record of trace `trace` (refused unless it is a trace id).
fn send(trace: &str, line: Value) {
    if valid_id(trace, 32) {
        queue(format!("{trace}.jsonl"), line);
    }
}

/// Records this process dropped because its queue was full.
pub fn dropped() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Wait (at most `timeout`) until what was queued is written; nothing to wait for when nothing was ever queued.
pub fn flush(timeout: Duration) {
    let Some(tx) = TX.get() else { return };
    let until = std::time::Instant::now() + timeout;
    let (done, wait) = sync_channel(1);
    let mut msg = Msg::Flush(done);
    loop {
        match tx.try_send(msg) {
            Ok(()) => break,
            Err(std::sync::mpsc::TrySendError::Full(back)) if std::time::Instant::now() < until => {
                msg = back;
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return,
        }
    }
    let _ = wait.recv_timeout(until.saturating_duration_since(std::time::Instant::now()));
}

/// A day's detail at most: past it, only spans and the index are kept, events dropped (counted).
const MAX_DAY_BYTES: u64 = 512 << 20;
/// Lines written at once, under one lock.
const BATCH: usize = 1000;

fn write_loop(rx: Receiver<Msg>) {
    let mut swept: HashSet<(PathBuf, String)> = HashSet::new();
    // Each day folder's size as this process knows it (measured when first written to, then counted).
    let mut sizes: HashMap<PathBuf, u64> = HashMap::new();
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        while batch.len() < BATCH {
            match rx.try_recv() {
                Ok(m) => batch.push(m),
                Err(_) => break,
            }
        }
        // By day folder, then file: one lock and one write a file.
        let mut out: BTreeMap<PathBuf, BTreeMap<String, String>> = BTreeMap::new();
        let mut flushes = Vec::new();
        for m in batch {
            match m {
                Msg::Flush(done) => flushes.push(done),
                Msg::Line { dir, day, file, line } => {
                    if swept.insert((dir.clone(), day.clone())) {
                        sweep(&dir, Local::now().date_naive());
                    }
                    let day_dir = dir.join(&day);
                    let size = sizes.entry(day_dir.clone()).or_insert_with(|| dir_size(&day_dir));
                    if *size > MAX_DAY_BYTES && line["t"] == "event" {
                        DROPPED.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    let text = format!("{line}\n");
                    *size += text.len() as u64;
                    out.entry(day_dir).or_default().entry(file).or_default().push_str(&text);
                }
            }
        }
        for (day_dir, files) in out {
            append(&day_dir, &files);
        }
        for done in flushes {
            let _ = done.send(());
        }
    }
}

fn dir_size(d: &Path) -> u64 {
    std::fs::read_dir(d).into_iter().flatten().flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()
}

fn append(day_dir: &Path, files: &BTreeMap<String, String>) {
    if std::fs::create_dir_all(day_dir).is_err() {
        return;
    }
    let Ok(_lock) = crate::plugins::wait_lock_for(&day_dir.join(".lock"), "Echo's traces", Duration::from_secs(5)) else { return };
    for (file, text) in files {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(day_dir.join(file)) {
            let _ = f.write_all(text.as_bytes());
        }
    }
}

/// A day `KEEP_DAYS` old or older keeps only its numbers (its index); one `NUMBERS_DAYS` old or older goes.
fn sweep(dir: &Path, today: NaiveDate) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(d) = NaiveDate::parse_from_str(&name, "%Y-%m-%d") else { continue };
        let age = (today - d).num_days();
        if age >= NUMBERS_DAYS {
            let _ = std::fs::remove_dir_all(e.path());
        } else if age >= KEEP_DAYS {
            for f in std::fs::read_dir(e.path()).into_iter().flatten().flatten() {
                if f.file_name() != INDEX && f.file_name() != ".lock" {
                    let _ = std::fs::remove_file(f.path());
                }
            }
        }
    }
}

fn new_id(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    if getrandom::fill(&mut b).is_err() || b.iter().all(|x| *x == 0) {
        let n = Utc::now().timestamp_nanos_opt().unwrap_or(1) as u128 ^ (std::process::id() as u128) << 64;
        b = n.to_le_bytes()[..bytes.min(16)].to_vec();
        b.resize(bytes, 1);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `n` lowercase hex digits, not all zero (a trace id: 32, a span id: 16).
fn valid_id(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) && s.bytes().any(|c| c != b'0')
}

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

// ── Echo's spans and events ──────────────────────────────────────────────────────────────────

thread_local! {
    /// This thread's open spans, innermost last: (trace, span, name).
    static CURRENT: RefCell<Vec<(String, String, String)>> = const { RefCell::new(Vec::new()) };
    /// Attributes added to an open span of this thread (`annotate`), by its id.
    static EXTRA: RefCell<HashMap<String, Map<String, Value>>> = RefCell::new(HashMap::new());
    /// The ended children of this thread's open root spans, by trace: written when the root ends, dropped with it
    /// when it is discarded (a look that found nothing leaves nothing).
    static HELD: RefCell<HashMap<String, Vec<Value>>> = RefCell::new(HashMap::new());
}

/// What the Claude Code runs of a trace used, summed for its index line: runs, tokens, cost, models.
#[derive(Default)]
struct Totals {
    runs: u64,
    tokens: u64,
    /// Of `tokens`, read from the prompt cache.
    cached: u64,
    /// Requests to other services (`http.request` spans).
    requests: u64,
    cost_usd: f64,
    models: Vec<String>,
}

static TOTALS: Mutex<BTreeMap<String, Totals>> = Mutex::new(BTreeMap::new());

/// A span of Echo's own: the current one of its thread until it ends (dropped, or `discard`ed: then not kept).
pub struct Span {
    trace: String,
    id: String,
    parent: Option<String>,
    name: String,
    start: DateTime<Utc>,
    attrs: Map<String, Value>,
    open: bool,
    /// It stays on the thread it started on (its place in that thread's spans).
    _here: std::marker::PhantomData<*const ()>,
}

/// The innermost open span of this thread: (trace, span, name).
fn current() -> Option<(String, String, String)> {
    CURRENT.with(|c| c.borrow().last().cloned())
}

/// The name of this thread's innermost open span.
pub fn current_name() -> Option<String> {
    current().map(|(_, _, n)| n)
}

/// The job this thread runs: its outermost span's, when that is a job's (`job.<id>`).
fn current_job() -> Option<String> {
    CURRENT.with(|c| c.borrow().first().and_then(|(_, _, n)| n.strip_prefix("job.").map(str::to_owned)))
}

/// Start a span on this thread: a child of its current span, else a new trace.
pub fn start(name: &str, attrs: Value) -> Span {
    let at = current().map(|(t, s, _)| (t, s));
    begin(name, attrs, at)
}

/// Start a trace of its own on this thread, whatever span is open (a job of its own run inside another's pass): the
/// current span until it ends.
pub fn start_root(name: &str, attrs: Value) -> Span {
    begin(name, attrs, None)
}

fn begin(name: &str, attrs: Value, under: Option<(String, String)>) -> Span {
    let (trace, parent) = match under {
        Some((t, s)) => (t, Some(s)),
        None => (new_id(16), None),
    };
    let id = new_id(8);
    CURRENT.with(|c| c.borrow_mut().push((trace.clone(), id.clone(), name.to_string())));
    Span { trace, id, parent, name: name.to_string(), start: Utc::now(), attrs: obj(attrs), open: true, _here: std::marker::PhantomData }
}

impl Span {
    pub fn set(&mut self, k: &str, v: impl Into<Value>) {
        self.attrs.insert(k.to_string(), v.into());
    }

    /// Not kept, nor the spans under it it held: a look that found nothing to do.
    pub fn discard(mut self) {
        self.pop();
        self.open = false;
        if self.parent.is_none() {
            HELD.with(|h| h.borrow_mut().remove(&self.trace));
            TOTALS.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.trace);
            return;
        }
        // A step's: the spans held under it go with it.
        HELD.with(|h| {
            if let Some(held) = h.borrow_mut().get_mut(&self.trace) {
                let mut gone: HashSet<String> = HashSet::from([self.id.clone()]);
                loop {
                    let more: Vec<String> = held.iter().filter(|l| l["parent"].as_str().is_some_and(|p| gone.contains(p))).filter_map(|l| l["span"].as_str().map(str::to_owned)).filter(|s| !gone.contains(s)).collect();
                    if more.is_empty() {
                        break;
                    }
                    gone.extend(more);
                }
                held.retain(|l| !l["span"].as_str().is_some_and(|s| gone.contains(s)));
            }
        });
    }

    fn pop(&mut self) {
        CURRENT.with(|c| {
            let mut c = c.borrow_mut();
            if let Some(i) = c.iter().rposition(|(_, s, _)| *s == self.id) {
                c.truncate(i);
            }
        });
        if let Some(extra) = EXTRA.with(|e| e.borrow_mut().remove(&self.id)) {
            self.attrs.extend(extra);
        }
    }

    fn close(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        self.pop();
        // Its trace's root still open on this thread: held until it ends.
        let root_open = CURRENT.with(|c| c.borrow().iter().any(|(t, _, _)| *t == self.trace));
        // Its Claude Code runs that gave no result (stopped, failed to start) end with it.
        let orphans: Vec<String> = RUNS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|r| r.parent.as_deref() == Some(self.id.as_str())).map(|r| r.id.clone()).collect();
        for r in orphans {
            end_run(&r, None);
        }
        let end = Utc::now();
        if self.name == "http.request" {
            TOTALS.lock().unwrap_or_else(|e| e.into_inner()).entry(self.trace.clone()).or_default().requests += 1;
        }
        let line = json!({"t": "span", "source": "echo", "trace": self.trace, "span": self.id, "parent": self.parent, "name": self.name,
                          "start": ts(self.start), "end": ts(end), "attrs": self.attrs});
        if root_open {
            HELD.with(|h| h.borrow_mut().entry(self.trace.clone()).or_default().push(line));
            return;
        }
        for held in HELD.with(|h| h.borrow_mut().remove(&self.trace)).unwrap_or_default() {
            send(&self.trace, held);
        }
        send(&self.trace, line);
        if self.parent.is_none() {
            index(&self.trace, &self.name, self.start, end, &self.attrs);
        }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        self.close();
    }
}

/// `f` as a span `name` of this thread (`attrs`), with how it ended: `ok`, and the error when it failed. Its answer as
/// it is.
pub fn traced<T>(name: &str, attrs: Value, f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    let mut s = start(name, attrs);
    let out = f();
    s.set("ok", out.is_ok());
    if let Err(e) = &out {
        s.set("error", format!("{e:#}"));
    }
    out
}

/// What a request to another service is: its service, method, host, path and query (never a header: no token).
pub fn outbound(service: &str, method: &str, url: &str) -> Value {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let (host, path) = rest.split_once('/').map_or((rest, String::from("/")), |(h, p)| (h, format!("/{p}")));
    let (path, query) = path.split_once('?').map_or((path.as_str(), ""), |(p, q)| (p, q));
    json!({"service": service, "method": method, "host": host, "path": path, "query": query})
}

/// A model call made without Claude Code (a local profile's server) used this: counted in its trace's totals.
pub fn count_usage(model: &str, v: &Value) {
    let Some((trace, _, _)) = current() else { return };
    let u = &v["usage"];
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    let mut totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
    let t = totals.entry(trace).or_default();
    t.tokens += n("input_tokens") + n("output_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens");
    t.cached += n("cache_read_input_tokens");
    t.cost_usd += v["total_cost_usd"].as_f64().unwrap_or(0.0);
    if !model.is_empty() && !t.models.iter().any(|x| x == model) {
        t.models.push(model.to_string());
    }
}

/// Add attributes to this thread's current span (kept when it ends).
pub fn annotate(attrs: Value) {
    if let Some((_, s, _)) = current() {
        EXTRA.with(|e| e.borrow_mut().entry(s).or_default().extend(obj(attrs)));
    }
}

/// A span for something that already happened (`started` … now): a child of this thread's current span, else a trace.
pub fn record_span(name: &str, started: DateTime<Local>, attrs: Value) {
    let mut s = start(name, attrs);
    s.start = started.with_timezone(&Utc);
}

/// A trace that ended: its line in the day's index, with what its Claude Code runs used.
fn index(trace: &str, name: &str, start: DateTime<Utc>, end: DateTime<Utc>, attrs: &Map<String, Value>) {
    let t = TOTALS.lock().unwrap_or_else(|e| e.into_inner()).remove(trace).unwrap_or_default();
    let job = attrs.get("echo.job").cloned().or_else(|| name.strip_prefix("job.").map(|j| json!(j)));
    let line = json!({"id": trace, "name": name, "kind": kind_of(name), "job": job, "start": ts(start), "end": ts(end), "duration_ms": (end - start).num_milliseconds().max(0),
        "ok": attrs.get("ok").cloned().unwrap_or(json!(true)), "summary": attrs.get("summary"), "error": attrs.get("error"),
        "runs": t.runs, "tokens": t.tokens, "cached": t.cached, "requests": t.requests, "cost_usd": (t.cost_usd * 1e6).round() / 1e6, "model": t.models.join(", ")});
    queue(INDEX.into(), line);
}

/// A trace's kind, from its root's name: `job`, `claude`, `request`, `model`, `command`, else the name's first word
/// (`transcription`, `app`).
fn kind_of(name: &str) -> &str {
    match name.split('.').next().unwrap_or("") {
        "job" => "job",
        "claude" => "claude",
        "http" => "request",
        "model" => "model",
        "cli" => "command",
        other => other,
    }
}

// ── Claude Code runs ─────────────────────────────────────────────────────────────────────────

/// A Claude Code run started, not yet ended (by its result, or the next run its thread starts).
struct Run {
    trace: String,
    id: String,
    parent: Option<String>,
    start: DateTime<Utc>,
    attrs: Map<String, Value>,
}

static RUNS: Mutex<Vec<Run>> = Mutex::new(Vec::new());

/// The traces of the runs this process started (open or recently ended), by when: the receiver files only theirs.
static KNOWN: Mutex<BTreeMap<String, DateTime<Utc>>> = Mutex::new(BTreeMap::new());

/// A run of this process has this trace (started within two days: a live notes session lasts a call).
pub(crate) fn known(trace: &str) -> bool {
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).get(trace).is_some_and(|t| *t > Utc::now() - chrono::Duration::days(2))
}

thread_local! {
    /// The run this thread started last (`claude_run`), while it is open.
    static LAST_RUN: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Variables that name Claude Code's telemetry: when any is set where a run reads it, its telemetry is someone's.
pub fn telemetry_var(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let n = if cfg!(windows) { upper.as_str() } else { name };
    n.starts_with("OTEL_") || n == "CLAUDE_CODE_ENABLE_TELEMETRY" || n == "CLAUDE_CODE_ENHANCED_TELEMETRY_BETA"
}

/// The settings files whose `env` a run of `p` in `cwd` may read: the managed ones, the profile's (or the user's own)
/// and the folder's project settings.
fn settings_files(p: &Profile, cwd: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "macos")]
    let managed = PathBuf::from("/Library/Application Support/ClaudeCode");
    #[cfg(windows)]
    let managed = PathBuf::from(r"C:\Program Files\ClaudeCode");
    #[cfg(not(any(target_os = "macos", windows)))]
    let managed = PathBuf::from("/etc/claude-code");
    v.push(managed.join("managed-settings.json"));
    let mut dropins: Vec<PathBuf> = std::fs::read_dir(managed.join("managed-settings.d")).into_iter().flatten().flatten().map(|e| e.path()).collect();
    dropins.sort();
    v.extend(dropins);
    let user = if p.kind.managed() {
        p.config_folder()
    } else {
        std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| paths::user_home().join(".claude"))
    };
    v.push(user.join("settings.json"));
    v.push(cwd.join(".claude").join("settings.json"));
    v.push(cwd.join(".claude").join("settings.local.json"));
    v
}

/// Does something already configure Claude Code's telemetry for a run of `p` in `cwd`: this process's environment, or a
/// settings file's `env`? (The login shell's start-up files are checked in its script.)
pub fn configured_elsewhere(p: &Profile, cwd: &Path) -> bool {
    std::env::vars_os().any(|(k, _)| k.to_str().is_some_and(telemetry_var))
        || settings_files(p, cwd).iter().any(|f| {
            std::fs::read(f)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_some_and(|v| v["env"].as_object().is_some_and(|e| e.keys().any(|k| telemetry_var(k))))
        })
}

/// What a run's arguments are, with the text of the instruction files they name.
fn args_attrs(args: &[String]) -> Value {
    let mut files = Map::new();
    for w in args.windows(2) {
        if matches!(w[0].as_str(), "--system-prompt-file" | "--append-system-prompt-file") {
            if let Ok(t) = std::fs::read_to_string(&w[1]) {
                files.insert(w[0].trim_start_matches('-').to_string(), json!(t));
            }
        }
    }
    json!({"args": args, "files": files})
}

/// A Claude Code print run of `p` in `cwd` with `args` is about to start on this thread: its span (a child of this
/// thread's current span, else a trace of its own), and the variables that point Claude Code's telemetry at this
/// process — none when something else configures it, or the receiver can't listen.
pub fn claude_run(p: &Profile, cwd: &Path, args: &[String]) -> Vec<(&'static str, String)> {
    if let Some(prev) = LAST_RUN.with(|l| l.borrow_mut().take()) {
        end_run(&prev, None);
    }
    let (trace, parent) = match current() {
        Some((t, s, _)) => (t, Some(s)),
        None => (new_id(16), None),
    };
    let id = new_id(8);
    let job = current_job();
    // A wrapper (a company's command in place of `claude`) may set telemetry of its own where Echo can't see it: left
    // as it is.
    let wrapper = p.kind == crate::ai::Kind::Wrapper;
    let elsewhere = !wrapper && configured_elsewhere(p, cwd);
    let mut vars = Vec::new();
    if !elsewhere && !wrapper {
        if let Some(endpoint) = receiver::endpoint() {
            // W3C baggage-style values: anything but a plain word percent-encoded.
            let enc = |v: &str| v.bytes().map(|b| if b.is_ascii_alphanumeric() || b"._-".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
            let job_attr = job.as_deref().map(|j| format!(",echo.job={}", enc(j))).unwrap_or_default();
            vars = vec![
                ("CLAUDE_CODE_ENABLE_TELEMETRY", "1".to_string()),
                ("CLAUDE_CODE_ENHANCED_TELEMETRY_BETA", "1".into()),
                ("OTEL_TRACES_EXPORTER", "otlp".into()),
                ("OTEL_LOGS_EXPORTER", "otlp".into()),
                ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/json".into()),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint),
                ("OTEL_LOGS_EXPORT_INTERVAL", "2000".into()),
                ("OTEL_RESOURCE_ATTRIBUTES", format!("echo.trace={trace},echo.span={id}{job_attr}")),
                ("TRACEPARENT", format!("00-{trace}-{id}-01")),
            ];
        }
    }
    let telemetry = if wrapper { "the wrapper's own" } else if elsewhere { "configured elsewhere" } else if vars.is_empty() { "off" } else { "echo" };
    let mut attrs = obj(args_attrs(args));
    attrs.insert("echo.job".into(), json!(job));
    attrs.insert("profile".into(), json!(p.id));
    attrs.insert("cwd".into(), json!(cwd.display().to_string()));
    attrs.insert("echo.telemetry".into(), json!(telemetry));
    // A run left without a result two days ago won't get one: it ends now.
    let stale = Utc::now() - chrono::Duration::days(2);
    let old: Vec<String> = RUNS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|r| r.start < stale).map(|r| r.id.clone()).collect();
    for r in old {
        end_run(&r, None);
    }
    {
        let mut known = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
        known.retain(|_, t| *t > stale);
        known.insert(trace.clone(), Utc::now());
    }
    RUNS.lock().unwrap_or_else(|e| e.into_inner()).push(Run { trace, id: id.clone(), parent, start: Utc::now(), attrs });
    LAST_RUN.with(|l| *l.borrow_mut() = Some(id));
    vars
}

/// The run this thread started last, while it is open: for a thread that reads its stream elsewhere (`claude_event_of`).
pub fn last_run() -> Option<String> {
    LAST_RUN.with(|l| l.borrow().clone())
}

/// The run this thread started last goes on beyond its job (the live notes session, read on its own thread): its
/// own trace's root from now, never ended by this thread's next run or its job's end.
pub fn detach_last_run() -> Option<String> {
    let run = LAST_RUN.with(|l| l.borrow_mut().take())?;
    on_run(&run, |r| r.parent = None);
    Some(run)
}

fn on_run(run: &str, f: impl FnOnce(&mut Run)) {
    if let Some(r) = RUNS.lock().unwrap_or_else(|e| e.into_inner()).iter_mut().find(|r| r.id == run) {
        f(r);
    }
}

/// What the run this thread started last was given on stdin (or, through a wrapper, as its prompt).
pub fn claude_input(text: &str) {
    if let Some(run) = last_run() {
        let mut at = None;
        on_run(&run, |r| at = Some(r.trace.clone()));
        if let Some(trace) = at {
            send(&trace, json!({"t": "event", "source": "echo", "trace": trace, "span": run, "name": "claude.input", "at": ts(Utc::now()), "attrs": {"text": text}}));
        }
    }
}

/// A stream-json event of the run this thread started last.
pub fn claude_event(ev: &Value) {
    if let Some(run) = last_run() {
        claude_event_of(&run, ev);
    }
}

/// A stream-json event of run `run`, kept whole; its start names the model, its result ends the run.
pub fn claude_event_of(run: &str, ev: &Value) {
    // A streamed run's partial text (`--include-partial-messages`) is in its whole messages too.
    if ev["type"] == "stream_event" {
        return;
    }
    let mut trace = None;
    on_run(run, |r| {
        trace = Some(r.trace.clone());
        if ev["type"] == "system" && ev["subtype"] == "init" {
            for (k, from) in [("model", "model"), ("session", "session_id"), ("claude_code_version", "claude_code_version"), ("permission_mode", "permissionMode")] {
                r.attrs.insert(k.into(), ev[from].clone());
            }
        }
    });
    let Some(trace) = trace else { return };
    let kind = ev["type"].as_str().unwrap_or("event");
    let name = match ev["subtype"].as_str() {
        Some(s) => format!("claude.{kind}.{s}"),
        None => format!("claude.{kind}"),
    };
    send(&trace, json!({"t": "event", "source": "echo", "trace": trace, "span": run, "name": name, "at": ts(Utc::now()), "attrs": ev}));
    if ev["type"] == "result" {
        end_run(run, Some(ev));
    }
}

/// The run ended: its span, with how it ended and what it used (`result`), else as one that gave none.
fn end_run(run: &str, result: Option<&Value>) {
    let r = {
        let mut runs = RUNS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(i) = runs.iter().position(|r| r.id == run) else { return };
        runs.remove(i)
    };
    LAST_RUN.with(|l| {
        let mut l = l.borrow_mut();
        if l.as_deref() == Some(run) {
            *l = None;
        }
    });
    let mut attrs = r.attrs;
    let (mut tokens, mut cached) = (0, 0);
    let mut cost = 0.0;
    match result {
        Some(v) => {
            let u = &v["usage"];
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            tokens = n("input_tokens") + n("output_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens");
            cached = n("cache_read_input_tokens");
            cost = v["total_cost_usd"].as_f64().unwrap_or(0.0);
            attrs.insert("ok".into(), json!(v["is_error"] != true));
            for (k, val) in [("input", n("input_tokens")), ("output", n("output_tokens")), ("cache_read", n("cache_read_input_tokens")), ("cache_write", n("cache_creation_input_tokens"))] {
                attrs.insert(k.into(), json!(val));
            }
            attrs.insert("cost_usd".into(), json!(cost));
            attrs.insert("turns".into(), v["num_turns"].clone());
            attrs.insert("duration_ms".into(), v["duration_ms"].clone());
        }
        None => {
            attrs.insert("ok".into(), json!(false));
            attrs.insert("error".into(), json!("Claude Code ended without a result"));
        }
    }
    {
        let mut totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
        let t = totals.entry(r.trace.clone()).or_default();
        t.runs += 1;
        t.tokens += tokens;
        t.cached += cached;
        t.cost_usd += cost;
        if let Some(m) = attrs.get("model").and_then(Value::as_str).filter(|m| !m.is_empty()) {
            if !t.models.iter().any(|x| x == m) {
                t.models.push(m.to_string());
            }
        }
    }
    let end = Utc::now();
    send(&r.trace, json!({"t": "span", "source": "echo", "trace": r.trace, "span": r.id, "parent": r.parent, "name": "claude.run",
                          "start": ts(r.start), "end": ts(end), "attrs": attrs}));
    if r.parent.is_none() {
        index(&r.trace, "claude.run", r.start, end, &attrs);
    }
}

// ── reading ──────────────────────────────────────────────────────────────────────────────────

fn day_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(paths::config_dir().join(DIR))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| NaiveDate::parse_from_str(n, "%Y-%m-%d").is_ok()))
        .collect();
    v.sort();
    v.reverse();
    v
}

fn lines(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// `echoctl traces`: the traces of the last `days` days (today included, at most `NUMBERS_DAYS`), newest first, at most
/// `limit`; of one job, of one kind. Each says whether its detail is still kept (`detail`: younger than `KEEP_DAYS`).
pub fn list(days: i64, job: Option<&str>, kind: Option<&str>, limit: usize) -> Value {
    let today = Local::now().date_naive();
    let oldest = today - chrono::Duration::days(days.clamp(1, NUMBERS_DAYS) - 1);
    let mut out: Vec<Value> = Vec::new();
    for d in day_dirs() {
        let day = d.file_name().and_then(|n| n.to_str()).and_then(|n| NaiveDate::parse_from_str(n, "%Y-%m-%d").ok());
        if day.is_none_or(|x| x < oldest) {
            continue;
        }
        let detail = day.is_some_and(|x| (today - x).num_days() < KEEP_DAYS);
        let mut of_day: Vec<Value> = lines(&d.join(INDEX))
            .into_iter()
            .map(|mut v| {
                v["detail"] = json!(detail);
                // A line written before kinds: its kind from its name.
                if v["kind"].is_null() {
                    v["kind"] = json!(kind_of(v["name"].as_str().unwrap_or("")));
                }
                v
            })
            .filter(|v| job.is_none_or(|j| v["job"] == j) && kind.is_none_or(|k| v["kind"] == k))
            .collect();
        of_day.reverse();
        out.extend(of_day);
    }
    out.sort_by(|a, b| b["end"].as_str().cmp(&a["end"].as_str()));
    out.truncate(limit);
    json!({"traces": out, "kept_days": KEEP_DAYS, "numbers_days": NUMBERS_DAYS, "dropped": dropped()})
}

/// One trace's records as written, from every day it has lines in (a run across midnight), oldest day first.
pub(crate) fn records(id: &str) -> Vec<Value> {
    if !valid_id(id, 32) {
        return Vec::new();
    }
    day_dirs().into_iter().rev().flat_map(|d| lines(&d.join(format!("{id}.jsonl")))).collect()
}

/// `echoctl traces show <id>`: one trace's spans, nested, each with its events in order; events of no span it has last.
pub fn show(id: &str) -> anyhow::Result<Value> {
    let id = &id.to_ascii_lowercase();
    if !valid_id(id, 32) {
        anyhow::bail!("“{id}” isn't a trace id (32 hex digits, as `echoctl traces` lists them)");
    }
    let mut recs = records(id);
    if recs.is_empty() {
        anyhow::bail!("no trace “{id}” (a trace's detail is kept {KEEP_DAYS} days, its numbers {NUMBERS_DAYS})");
    }
    let at = |v: &Value| v["start"].as_str().or(v["at"].as_str()).unwrap_or("").to_string();
    recs.sort_by_key(at);
    let mut spans: Vec<Value> = recs.iter().filter(|r| r["t"] == "span").cloned().collect();
    let ids: HashSet<String> = spans.iter().filter_map(|s| s["span"].as_str().map(str::to_owned)).collect();
    let mut events: HashMap<String, Vec<Value>> = HashMap::new();
    let mut loose = Vec::new();
    for e in recs.iter().filter(|r| r["t"] == "event") {
        match e["span"].as_str().filter(|s| ids.contains(*s)) {
            Some(s) => events.entry(s.to_string()).or_default().push(e.clone()),
            None => loose.push(e.clone()),
        }
    }
    for s in &mut spans {
        let mine = events.remove(s["span"].as_str().unwrap_or("")).unwrap_or_default();
        s["events"] = json!(mine);
    }
    // Nested: each span under its parent, when the trace has it.
    let mut children: HashMap<String, Vec<Value>> = HashMap::new();
    let mut roots = Vec::new();
    for s in spans {
        match s["parent"].as_str().filter(|p| ids.contains(*p)) {
            Some(p) => children.entry(p.to_string()).or_default().push(s),
            None => roots.push(s),
        }
    }
    fn nest(mut s: Value, children: &mut HashMap<String, Vec<Value>>) -> Value {
        let kids = children.remove(s["span"].as_str().unwrap_or("")).unwrap_or_default();
        s["children"] = json!(kids.into_iter().map(|k| nest(k, children)).collect::<Vec<_>>());
        s
    }
    let roots: Vec<Value> = roots.into_iter().map(|r| nest(r, &mut children)).collect();
    Ok(json!({"id": id, "spans": roots, "events": loose}))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::testenv;

    /// The telemetry variables of this process's environment taken out for a test, put back when it ends.
    struct NoTelemetryEnv(Vec<(std::ffi::OsString, std::ffi::OsString)>);

    impl NoTelemetryEnv {
        fn take() -> Self {
            let found: Vec<_> = std::env::vars_os().filter(|(k, _)| k.to_str().is_some_and(telemetry_var)).collect();
            for (k, _) in &found {
                std::env::remove_var(k);
            }
            NoTelemetryEnv(found)
        }
    }

    impl Drop for NoTelemetryEnv {
        fn drop(&mut self) {
            for (k, v) in &self.0 {
                std::env::set_var(k, v);
            }
        }
    }

    fn var<'a>(vars: &'a [(&'static str, String)], k: &str) -> Option<&'a str> {
        vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str())
    }

    fn result(tokens: u64) -> Value {
        json!({"type": "result", "is_error": false, "usage": {"input_tokens": tokens, "output_tokens": 5, "cache_read_input_tokens": 100}, "total_cost_usd": 0.01, "num_turns": 2})
    }

    #[test]
    fn a_job_run_is_a_trace_with_its_claude_code_run_its_input_and_its_stream_under_it() {
        let (_env, root) = testenv::folders("telemetry-job");
        let _clean = NoTelemetryEnv::take();
        let p = crate::ai::fake::profile("work", None, crate::ai::fake::SIGNED_OUT);
        let system = root.join("system.md");
        std::fs::write(&system, "The fixed instructions.").unwrap();
        let run = crate::jobs::begin("needs");
        let args: Vec<String> = ["-p", "the prompt", "--append-system-prompt-file", system.to_str().unwrap()].map(String::from).to_vec();
        let vars = claude_run(&p, &root, &args);
        claude_input("what went in on stdin");
        claude_event(&json!({"type": "system", "subtype": "init", "model": "claude-haiku-5-5", "session_id": "s1"}));
        claude_event(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "the reply"}]}}));
        claude_event(&result(10));
        run.done(Some("checked".into()));
        flush(Duration::from_secs(5));

        let l = list(KEEP_DAYS, None, None, 10);
        let t = &l["traces"][0];
        assert_eq!(l["traces"].as_array().unwrap().len(), 1, "{l}");
        assert_eq!((t["name"].clone(), t["job"].clone(), t["ok"].clone(), t["summary"].clone()), (json!("job.needs"), json!("needs"), json!(true), json!("checked")));
        assert_eq!((t["runs"].clone(), t["tokens"].clone(), t["cost_usd"].clone(), t["model"].clone()), (json!(1), json!(115), json!(0.01), json!("claude-haiku-5-5")));
        let id = t["id"].as_str().unwrap();
        // Claude Code reports to this process, its root span under the run's.
        assert!(var(&vars, "TRACEPARENT").is_some_and(|v| v.starts_with(&format!("00-{id}-"))), "{vars:?}");
        assert!(var(&vars, "OTEL_EXPORTER_OTLP_ENDPOINT").is_some_and(|v| v.starts_with("http://127.0.0.1:")));
        assert_eq!((var(&vars, "CLAUDE_CODE_ENABLE_TELEMETRY"), var(&vars, "OTEL_EXPORTER_OTLP_PROTOCOL")), (Some("1"), Some("http/json")));
        assert!(var(&vars, "OTEL_RESOURCE_ATTRIBUTES").is_some_and(|v| v.contains("echo.job=needs")));
        assert!(!vars.iter().any(|(k, _)| k.starts_with("OTEL_LOG_")), "Claude Code's own text logging stays off");

        let shown = show(id).unwrap();
        let job = &shown["spans"][0];
        assert_eq!(job["name"], "job.needs");
        let claude = &job["children"][0];
        assert_eq!((claude["name"].clone(), claude["attrs"]["echo.telemetry"].clone(), claude["attrs"]["model"].clone()), (json!("claude.run"), json!("echo"), json!("claude-haiku-5-5")));
        assert_eq!(claude["attrs"]["args"][1], "the prompt");
        assert_eq!(claude["attrs"]["files"]["append-system-prompt-file"], "The fixed instructions.");
        let names: Vec<&str> = claude["events"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["claude.input", "claude.system.init", "claude.assistant", "claude.result"]);
        assert_eq!(claude["events"][0]["attrs"]["text"], "what went in on stdin");
        assert_eq!(claude["events"][2]["attrs"]["message"]["content"][0]["text"], "the reply");
    }

    #[test]
    fn a_job_pass_that_did_nothing_leaves_no_trace() {
        let (_env, _root) = testenv::folders("telemetry-nothing");
        crate::jobs::begin("needs").done(None);
        flush(Duration::from_secs(5));
        assert_eq!(list(KEEP_DAYS, None, None, 10)["traces"], json!([]));
    }

    #[test]
    fn echo_sets_no_telemetry_when_something_else_configures_claude_codes() {
        let (_env, root) = testenv::folders("telemetry-elsewhere");
        let _clean = NoTelemetryEnv::take();
        let p = crate::ai::fake::profile("work", None, crate::ai::fake::SIGNED_OUT);
        let run = |cwd: &Path| {
            let vars = claude_run(&p, cwd, &["-p".into(), "x".into()]);
            claude_event(&result(1));
            vars
        };
        std::fs::create_dir_all(p.config_folder()).unwrap();
        std::fs::write(p.config_folder().join("settings.json"), r#"{"env": {"MY_OWN": "1"}}"#).unwrap();
        assert!(!run(&root).is_empty(), "a settings file without telemetry: Echo's");

        std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://collector.acme.example:4318");
        assert!(run(&root).is_empty(), "the environment's");
        std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");

        std::fs::write(p.config_folder().join("settings.json"), r#"{"env": {"CLAUDE_CODE_ENABLE_TELEMETRY": "1"}}"#).unwrap();
        assert!(run(&root).is_empty(), "the profile's settings'");
        std::fs::remove_file(p.config_folder().join("settings.json")).unwrap();

        let folder = root.join("project");
        std::fs::create_dir_all(folder.join(".claude")).unwrap();
        std::fs::write(folder.join(".claude/settings.local.json"), r#"{"env": {"OTEL_LOGS_EXPORTER": "otlp"}}"#).unwrap();
        assert!(run(&folder).is_empty(), "the folder's project settings'");
        assert!(!run(&root).is_empty(), "elsewhere: Echo's again");

        flush(Duration::from_secs(5));
        let said: Vec<Value> = list(KEEP_DAYS, None, None, 10)["traces"].as_array().unwrap().iter().map(|t| show(t["id"].as_str().unwrap()).unwrap()["spans"][0]["attrs"]["echo.telemetry"].clone()).collect();
        assert_eq!(said.iter().filter(|s| **s == "configured elsewhere").count(), 3, "{said:?}");
    }

    /// One HTTP request on `s` (to `host`, of `kind`), its status line back.
    fn post_as(s: &mut std::net::TcpStream, host: &str, kind: &str, path: &str, body: &Value) -> String {
        use std::io::{BufRead, Read};
        let b = body.to_string();
        s.write_all(format!("POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\r\n{b}", b.len()).as_bytes()).unwrap();
        let mut r = std::io::BufReader::new(s.try_clone().unwrap());
        let mut status = String::new();
        r.read_line(&mut status).unwrap();
        let mut h = String::new();
        while r.read_line(&mut h).unwrap() > 0 && h.trim() != "" {
            h.clear();
        }
        let mut body = [0u8; 2];
        r.read_exact(&mut body).unwrap();
        status.trim().to_string()
    }

    fn post(s: &mut std::net::TcpStream, path: &str, body: &Value) -> String {
        let host = s.peer_addr().unwrap().to_string();
        post_as(s, &host, "application/json", path, body)
    }

    /// A run started on this thread, ended at once: its trace and span (the receiver knows it).
    fn a_run(root: &Path) -> (String, String) {
        let p = crate::ai::fake::profile("work", None, crate::ai::fake::SIGNED_OUT);
        claude_run(&p, root, &["-p".into(), "x".into()]);
        let run = last_run().unwrap();
        let mut trace = String::new();
        on_run(&run, |r| trace = r.trace.clone());
        claude_event(&result(1));
        (trace, run)
    }

    #[test]
    fn the_receiver_files_claude_codes_spans_and_events_by_their_trace() {
        let (_env, root) = testenv::folders("telemetry-receiver");
        let _clean = NoTelemetryEnv::take();
        let (trace, span) = a_run(&root);
        let at = (Utc::now().timestamp_nanos_opt().unwrap()).to_string();
        let kv = |k: &str, v: &str| json!({"key": k, "value": {"stringValue": v}});
        let spans = json!({"resourceSpans": [{"resource": {"attributes": [kv("service.name", "claude-code")]}, "scopeSpans": [{"spans": [
            {"traceId": trace, "spanId": "00f067aa0ba902b7", "parentSpanId": span, "name": "claude_code.llm_request", "startTimeUnixNano": at, "endTimeUnixNano": at,
             "attributes": [kv("gen_ai.request.model", "claude-haiku-5-5"), {"key": "input_tokens", "value": {"intValue": "12"}}]},
            {"traceId": "not-a-trace", "spanId": "1", "name": "dropped", "startTimeUnixNano": at, "endTimeUnixNano": at}]}]}]});
        // A log record without its own ids is filed by what Echo gave the run.
        let logs = json!({"resourceLogs": [{"resource": {"attributes": [kv("echo.trace", &trace), kv("echo.span", &span)]}, "scopeLogs": [{"logRecords": [
            {"timeUnixNano": at, "body": {"stringValue": "claude_code.api_request"}, "attributes": [kv("event.name", "api_request"), {"key": "cost_usd", "value": {"doubleValue": 0.004}}]}]}]}]});
        let addr = receiver::endpoint().unwrap().trim_start_matches("http://").to_string();
        let mut s = std::net::TcpStream::connect(&addr).unwrap();
        // One connection, several requests (Claude Code keeps it open).
        assert_eq!(post(&mut s, "/v1/traces", &spans), "HTTP/1.1 200 OK");
        assert_eq!(post(&mut s, "/v1/logs", &logs), "HTTP/1.1 200 OK");
        assert_eq!(post(&mut s, "/v1/metrics", &json!({})), "HTTP/1.1 200 OK");
        assert_eq!(post(&mut s, "/elsewhere", &json!({})), "HTTP/1.1 404 Not Found");
        testenv::eventually("the records written", || {
            flush(Duration::from_secs(1));
            show(&trace).is_ok_and(|t| t.to_string().contains("api_request"))
        });
        let t = show(&trace).unwrap();
        // Under Echo's run: Claude Code's request (its parent) and its event (filed by the run's span).
        let run = &t["spans"][0];
        assert_eq!((run["name"].clone(), run["span"].clone()), (json!("claude.run"), json!(span)));
        let llm = &run["children"][0];
        assert_eq!((llm["source"].clone(), llm["name"].clone(), llm["parent"].clone()), (json!("claude-code"), json!("claude_code.llm_request"), json!(span)));
        assert_eq!((llm["attrs"]["gen_ai.request.model"].clone(), llm["attrs"]["input_tokens"].clone()), (json!("claude-haiku-5-5"), json!(12)));
        let ev = run["events"].as_array().unwrap().iter().find(|e| e["name"] == "api_request").unwrap();
        assert_eq!((ev["source"].clone(), ev["attrs"]["cost_usd"].clone()), (json!("claude-code"), json!(0.004)));
        let ours: Vec<&Value> = run["children"].as_array().unwrap().iter().filter(|s| s["source"] == "claude-code").collect();
        assert_eq!(ours.len(), 1, "the span with no trace isn't kept");
    }
    #[test]
    fn the_receiver_takes_only_claude_codes_json_for_runs_of_its_own() {
        let (_env, root) = testenv::folders("telemetry-refused");
        let _clean = NoTelemetryEnv::take();
        let (trace, _) = a_run(&root);
        let stranger = new_id(16);
        let at = Utc::now().timestamp_nanos_opt().unwrap().to_string();
        let span_of = |t: &str| json!({"resourceSpans": [{"resource": {}, "scopeSpans": [{"spans": [{"traceId": t, "spanId": "00f067aa0ba902b7", "name": "planted", "startTimeUnixNano": at, "endTimeUnixNano": at}]}]}]});
        let addr = receiver::endpoint().unwrap().trim_start_matches("http://").to_string();
        let mut s = std::net::TcpStream::connect(&addr).unwrap();
        assert_eq!(post(&mut s, "/v1/traces", &span_of(&stranger)), "HTTP/1.1 200 OK", "answered, not kept");
        assert_eq!(post_as(&mut s, &addr, "text/plain", "/v1/traces", &span_of(&trace)), "HTTP/1.1 400 Bad Request", "a web page's simple POST");
        assert_eq!(post_as(&mut s, "evil.example", "application/json", "/v1/traces", &span_of(&trace)), "HTTP/1.1 400 Bad Request", "another host");
        // Too big, or a line without end: the connection is dropped, nothing allocated for it.
        let mut big = std::net::TcpStream::connect(&addr).unwrap();
        big.write_all(format!("POST /v1/traces HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: 999999999\r\n\r\n").as_bytes()).unwrap();
        let mut long = std::net::TcpStream::connect(&addr).unwrap();
        let _ = long.write_all(&vec![b'a'; 40_000]);
        for c in [&mut big, &mut long] {
            c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut b = [0u8; 16];
            assert!(matches!(std::io::Read::read(c, &mut b), Ok(0) | Err(_)), "closed without an answer");
        }
        flush(Duration::from_secs(5));
        assert!(show(&stranger).is_err(), "a trace of no run of Echo's isn't kept");
        assert!(!show(&trace).unwrap().to_string().contains("planted"));
    }

    #[test]
    fn a_run_that_gives_no_result_ends_with_its_job_and_says_so() {
        let (_env, root) = testenv::folders("telemetry-no-result");
        let _clean = NoTelemetryEnv::take();
        let p = crate::ai::fake::profile("work", None, crate::ai::fake::SIGNED_OUT);
        let run = crate::jobs::begin("minutes");
        claude_run(&p, &root, &["-p".into(), "x".into()]);
        claude_event(&json!({"type": "system", "subtype": "init", "model": "claude-haiku-5-5"}));
        run.failed("stopped by its time limit");
        flush(Duration::from_secs(5));
        let t = &list(KEEP_DAYS, None, None, 10)["traces"][0];
        assert_eq!((t["ok"].clone(), t["runs"].clone(), t["error"].clone()), (json!(false), json!(1), json!("stopped by its time limit")));
        let c = &show(t["id"].as_str().unwrap()).unwrap()["spans"][0]["children"][0];
        assert_eq!((c["name"].clone(), c["attrs"]["ok"].clone(), c["attrs"]["error"].clone()), (json!("claude.run"), json!(false), json!("Claude Code ended without a result")));
        assert!(last_run().is_none(), "nothing left open on the thread");
    }

    #[test]
    fn a_session_detached_from_its_job_outlives_it_as_a_trace_of_its_own() {
        let (_env, root) = testenv::folders("telemetry-detached");
        let _clean = NoTelemetryEnv::take();
        let p = crate::ai::fake::profile("work", None, crate::ai::fake::SIGNED_OUT);
        let job = crate::jobs::begin("live_notes");
        claude_run(&p, &root, &["-p".into(), "x".into()]);
        let session = detach_last_run().unwrap();
        job.done(None);
        // The thread's next run doesn't end it; its own stream, read elsewhere, does.
        let a = a_run(&root);
        claude_event_of(&session, &json!({"type": "stream_event", "event": {"delta": {"text": "par"}}}));
        claude_event_of(&session, &result(7));
        flush(Duration::from_secs(5));
        let l = list(KEEP_DAYS, Some("live_notes"), None, 10);
        let t = &l["traces"][0];
        assert_eq!((t["name"].clone(), t["ok"].clone(), t["tokens"].clone()), (json!("claude.run"), json!(true), json!(112)), "{l}");
        let names: Vec<Value> = show(t["id"].as_str().unwrap()).unwrap()["spans"][0]["events"].as_array().unwrap().iter().map(|e| e["name"].clone()).collect();
        assert_eq!(names, [json!("claude.result")], "partial text isn't kept twice");
        let _ = a;
    }

    #[test]
    fn a_wrapper_profiles_runs_keep_their_own_telemetry() {
        let (_env, root) = testenv::folders("telemetry-wrapper");
        let _clean = NoTelemetryEnv::take();
        let mut p = crate::ai::fake::profile("work", None, crate::ai::fake::SIGNED_OUT);
        p.kind = crate::ai::Kind::Wrapper;
        assert!(claude_run(&p, &root, &["-p".into(), "x".into()]).is_empty());
        let run = last_run().unwrap();
        let mut said = Value::Null;
        on_run(&run, |r| said = r.attrs["echo.telemetry"].clone());
        assert_eq!(said, "the wrapper's own");
        claude_event(&result(1));
    }

    #[test]
    fn sending_never_waits_however_much_is_sent() {
        let (_env, _root) = testenv::folders("telemetry-queue");
        let trace = new_id(16);
        let t = std::time::Instant::now();
        for i in 0..(QUEUE * 2) {
            send(&trace, json!({"t": "event", "trace": trace, "n": i}));
        }
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        flush(Duration::from_secs(30));
    }

    #[test]
    fn a_days_detail_goes_after_seven_days_its_numbers_after_ninety() {
        let (_env, root) = testenv::folders("telemetry-sweep");
        let dir = root.join("t");
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        for d in ["2026-11-14", "2026-11-13", "2026-08-23", "2026-08-22", "notes"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
            std::fs::write(dir.join(d).join(INDEX), "{}\n").unwrap();
            std::fs::write(dir.join(d).join("0123456789abcdef0123456789abcdef.jsonl"), "{}\n").unwrap();
        }
        sweep(&dir, today);
        let files = |d: &str| {
            let mut v: Vec<String> = std::fs::read_dir(dir.join(d)).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
            v.sort();
            v
        };
        assert_eq!(files("2026-11-14").len(), 2, "6 days old: all of it");
        assert_eq!(files("2026-11-13"), [INDEX], "7 days old: its numbers only");
        assert_eq!(files("2026-08-23"), [INDEX], "89 days old: its numbers only");
        assert!(!dir.join("2026-08-22").exists(), "90 days old: gone");
        assert_eq!(files("notes").len(), 2, "not a day's: left alone");
    }

    #[test]
    fn a_run_past_seven_days_is_listed_by_its_numbers_without_its_detail() {
        let (_env, root) = testenv::folders("telemetry-numbers");
        let base = root.join("config/telemetry");
        let day = |n: i64| (Local::now().date_naive() - chrono::Duration::days(n)).format("%Y-%m-%d").to_string();
        for (n, id) in [(0, "a"), (10, "b"), (95, "c")] {
            std::fs::create_dir_all(base.join(day(n))).unwrap();
            std::fs::write(base.join(day(n)).join(INDEX), format!("{}\n", json!({"id": id, "name": "job.minutes", "job": "minutes", "tokens": 10, "end": format!("{}T12:00:00.000Z", day(n))}))).unwrap();
        }
        let l = list(NUMBERS_DAYS, None, None, 10);
        let got: Vec<(Value, Value)> = l["traces"].as_array().unwrap().iter().map(|t| (t["id"].clone(), t["detail"].clone())).collect();
        assert_eq!(got, [(json!("a"), json!(true)), (json!("b"), json!(false))], "95 days old: past the numbers too");
        assert_eq!((l["kept_days"].clone(), l["numbers_days"].clone()), (json!(7), json!(90)));
        assert_eq!(list(500, None, None, 10)["traces"].as_array().unwrap().len(), 2, "at most 90 days");
    }

    #[test]
    fn a_traced_step_answers_as_it_did_and_says_how_it_ended() {
        let (_env, _root) = testenv::folders("telemetry-traced");
        let job = crate::jobs::begin("prep");
        assert_eq!(traced("http.request", outbound("microsoft", "GET", "https://graph.microsoft.com/v1.0/me/events?$top=5"), || Ok(7)).unwrap(), 7);
        let e = traced("model.request", json!({"model": "virtual"}), || -> anyhow::Result<()> {
            count_usage("virtual", &json!({"usage": {"input_tokens": 30, "output_tokens": 12}}));
            anyhow::bail!("the server didn't answer")
        });
        assert_eq!(e.unwrap_err().to_string(), "the server didn't answer");
        job.done(Some("prepared".into()));
        flush(Duration::from_secs(5));
        let t = &list(KEEP_DAYS, None, Some("job"), 10)["traces"][0];
        assert_eq!((t["kind"].clone(), t["tokens"].clone(), t["model"].clone()), (json!("job"), json!(42), json!("virtual")));
        let kids = show(t["id"].as_str().unwrap()).unwrap()["spans"][0]["children"].clone();
        assert_eq!((kids[0]["name"].clone(), kids[0]["attrs"]["ok"].clone(), kids[0]["attrs"]["host"].clone(), kids[0]["attrs"]["path"].clone(), kids[0]["attrs"]["query"].clone()),
                   (json!("http.request"), json!(true), json!("graph.microsoft.com"), json!("/v1.0/me/events"), json!("$top=5")));
        assert_eq!((kids[1]["name"].clone(), kids[1]["attrs"]["ok"].clone(), kids[1]["attrs"]["error"].clone()), (json!("model.request"), json!(false), json!("the server didn't answer")));
    }

    #[test]
    fn a_look_that_found_nothing_leaves_nothing_and_a_discarded_step_takes_its_own_with_it() {
        let (_env, _root) = testenv::folders("telemetry-held");
        let req = |path: &str, ok: bool| traced("http.request", outbound("microsoft", "GET", &format!("https://graph.microsoft.com/v1.0{path}")), || if ok { Ok(()) } else { anyhow::bail!("ErrorItemNotFound") });
        let quiet = start("job.needs", json!({"echo.job": "needs"}));
        let _ = req("/me/messages", true);
        let _ = req("/me/messages/gone", false);
        quiet.discard();
        flush(Duration::from_secs(5));
        assert_eq!(list(KEEP_DAYS, None, None, 10)["traces"], json!([]), "its requests went with it, a handled failure too");

        let kept = start("job.thread_sync", json!({"echo.job": "thread_sync"}));
        let step = start("step", json!({}));
        let _ = req("/me/chats", true);
        step.discard();
        let _ = req("/me/messages", true);
        drop(kept);
        flush(Duration::from_secs(5));
        let t = &list(KEEP_DAYS, None, None, 10)["traces"][0];
        let shown = show(t["id"].as_str().unwrap()).unwrap();
        assert_eq!(shown["spans"].as_array().unwrap().len(), 1, "one root, no orphan: {shown}");
        let kids: Vec<Value> = shown["spans"][0]["children"].as_array().unwrap().iter().map(|k| k["attrs"]["path"].clone()).collect();
        assert_eq!(kids, [json!("/v1.0/me/messages")]);
    }

    #[test]
    fn a_job_of_its_own_inside_another_s_pass_is_kept_when_that_pass_is_not() {
        let (_env, _root) = testenv::folders("telemetry-own-root");
        let sync = start("job.thread_sync", json!({"echo.job": "thread_sync"}));
        let fetch = start_root("job.fetch", json!({"echo.job": "fetch"}));
        let _ = traced("http.request", outbound("microsoft", "GET", "https://graph.microsoft.com/v1.0/me/messages"), || Ok(()));
        crate::jobs::record("fetch", Local::now(), Ok("2 new items".into()), json!({}));
        drop(fetch);
        sync.discard();
        flush(Duration::from_secs(5));
        let l = list(KEEP_DAYS, None, None, 10);
        assert_eq!(l["traces"].as_array().unwrap().len(), 1, "{l}");
        let t = &l["traces"][0];
        assert_eq!((t["name"].clone(), t["job"].clone(), t["summary"].clone()), (json!("job.fetch"), json!("fetch"), json!("2 new items")));
        assert_eq!(show(t["id"].as_str().unwrap()).unwrap()["spans"][0]["children"][0]["name"], "http.request");
    }

    #[test]
    fn an_index_line_from_before_kinds_gets_its_kind_from_its_name() {
        let (_env, root) = testenv::folders("telemetry-old-index");
        let day = root.join("config/telemetry").join(Local::now().format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(day.join(INDEX), format!("{}\n", json!({"id": new_id(16), "name": "job.minutes", "job": "minutes", "end": ts(Utc::now())}))).unwrap();
        assert_eq!(list(KEEP_DAYS, None, Some("job"), 10)["traces"][0]["kind"], "job");
    }

    #[test]
    fn a_trace_is_asked_for_by_its_id_only() {
        let (_env, _root) = testenv::folders("telemetry-show");
        assert!(show("../../etc/passwd").unwrap_err().to_string().contains("isn't a trace id"));
        assert!(show(&new_id(16)).unwrap_err().to_string().contains("no trace"));
    }
}
