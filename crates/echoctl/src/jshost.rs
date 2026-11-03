//! Plugins' host halves (`docs/plugins-design.md`, v4): each enabled plugin's `main.js` run by QuickJS
//! inside Echo — also with the window closed — as an ES module whose `apply(ctx)` gets Echo's services.
//!
//! One host thread owns the runtime (QuickJS isn't shared between threads) and a context per plugin;
//! everything reaches it as a message: load, unload, a command call (from the window, ⌘K, the CLI), the
//! timers' tick, an async operation's outcome. The plugin side is a small prelude (`PRELUDE`) that turns
//! two Rust functions — `__host(op, json)` (at once) and `__hostAsync(op, json)` (a Promise, settled when
//! a worker thread is done) — into `ctx`. Commands may be `async`; their answer is JSON.
//!
//! Limits: 64 MB per runtime, and a call or tick that runs over 5 s of JavaScript is interrupted.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use rquickjs::{CatchResultExt, Context, Function, Module, Runtime};
use serde_json::{json, Value};

use crate::{paths, plugins};

/// How long one command may take before its caller is told so (the work may go on).
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// JavaScript running without a break longer than this is stopped.
const SLICE: Duration = Duration::from_secs(5);

/// The `ctx` every host half gets, built in JavaScript over `__host` / `__hostAsync`.
const PRELUDE: &str = r#"
(() => {
  const host = globalThis.__host, hostAsync = globalThis.__hostAsync
  const commands = {}, timers = [], pending = {}, handlers = {}
  const call = (op, arg) => {
    const out = JSON.parse(host(op, JSON.stringify(arg === undefined ? null : arg)))
    if (out && out.error) throw new Error(out.error)
    return out ? out.value : null
  }
  const later = (op, arg) => new Promise((resolve, reject) => { pending[hostAsync(op, JSON.stringify(arg === undefined ? null : arg))] = [resolve, reject] })
  globalThis.__settle = (token, json) => {
    const p = pending[token]; delete pending[token]; if (!p) return
    const out = JSON.parse(json); out.error ? p[1](new Error(out.error)) : p[0](out.value)
  }
  globalThis.__call = (id, name, args) => {
    const fn = commands[name]
    if (!fn) return host('reply', JSON.stringify({ call: id, error: `no command “${name}”` }))
    Promise.resolve().then(() => fn(JSON.parse(args))).then(
      (v) => host('reply', JSON.stringify({ call: id, value: v === undefined ? null : v })),
      (e) => host('reply', JSON.stringify({ call: id, error: String((e && e.message) || e) })))
  }
  globalThis.__tick = (now) => {
    for (const t of timers) if (now >= t.next) {
      t.next = now + t.every
      Promise.resolve().then(t.fn).catch((e) => call('log', ['error', `timer: ${(e && e.message) || e}`]))
    }
  }
  globalThis.__event = (name, payload) => {
    for (const fn of handlers[name] || []) Promise.resolve().then(() => fn(JSON.parse(payload))).catch((e) => call('log', ['error', `${name}: ${(e && e.message) || e}`]))
  }
  globalThis.__ctx = {
    id: call('id'),
    log: (...a) => call('log', ['info', a.map(String).join(' ')]),
    command: (name, fn) => { commands[name] = fn },
    every: (seconds, fn) => { timers.push({ every: Math.max(10, seconds) * 1000, next: 0, fn }) },
    on: (event, fn) => { (handlers[event] = handlers[event] || []).push(fn) },
    store: {
      read: (path) => call('store.read', path),
      write: (path, text) => call('store.write', [path, String(text)]),
      list: (dir) => call('store.list', dir || ''),
      remove: (path) => call('store.remove', path),
      path: (path) => call('store.path', path || ''),
    },
    settings: { get: (key) => call('settings.get', key) },
    package: { read: (path) => call('package.read', path), list: (dir) => call('package.list', dir || '') },
    markdown: (md) => call('markdown', String(md)),
    notify: (title, body) => call('notify', [String(title), String(body || '')]),
    inbox: { set: (key, item) => call('inbox.set', [key, item]), clear: (key) => call('inbox.clear', key) },
    profiles: () => call('profiles'),
    claude: {
      run: (opts) => later('claude.run', opts),
      /** Is a Claude Code plugin ("name@marketplace") installed for an AI profile ('' = the default)? */
      hasPlugin: (spec, profile) => later('claude.has_plugin', [spec, profile || '']),
      /** Install it there (user scope of that profile's Claude Code). */
      installPlugin: (spec, profile) => later('claude.install_plugin', [spec, profile || '']),
    },
    /** The Claude · Echo folder session listens to a Claude Code channel (the one the manifest names) while
     *  it is open; `env` is passed to it, `token: 'telegram'` adds the bot token Echo keeps. */
    sessions: { channel: (opts) => call('sessions.channel', opts), clearChannel: () => call('sessions.clear_channel') },
    system: { which: (name) => call('system.which', name) },
    deliver: { status: () => call('deliver.status'), send: (kind, msg) => later('deliver.send', [kind, msg]) },
  }
})()
"#;

enum Msg {
    Load(String, PathBuf),
    Unload(String),
    Call { plugin: String, command: String, args: String, reply: Sender<Result<Value, String>> },
    Event(String, String),
    Settle(String, u64, String),
}

struct Host {
    tx: Sender<Msg>,
    loaded: Mutex<BTreeMap<String, Result<(), String>>>,
}

static HOST: OnceLock<Host> = OnceLock::new();
/// The channels plugins asked the Echo-folder session to listen to: plugin → (channel plugin, env).
static CHANNELS: Mutex<BTreeMap<String, Channel>> = Mutex::new(BTreeMap::new());

#[derive(Debug, Clone)]
pub struct Channel {
    /// Claude Code's channel plugin, `name@marketplace`.
    pub spec: String,
    pub env: Vec<(String, String)>,
}

/// What the Echo-folder session adds for the enabled plugins' channels (`--channels` and their env).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn channels() -> Vec<Channel> {
    let on: Vec<String> = plugins::enabled().into_iter().map(|p| p.id).collect();
    CHANNELS.lock().map(|c| c.iter().filter(|(id, _)| on.contains(id)).map(|(_, ch)| ch.clone()).collect()).unwrap_or_default()
}

/// Async operations and command calls in flight, numbered.
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Off in a terminal (`echoctl plugin call`): the plugins' timers are the app's alone.
static TIMERS: AtomicBool = AtomicBool::new(true);

/// This process runs commands only, never the plugins' timers.
pub fn no_timers() {
    TIMERS.store(false, Ordering::Relaxed);
}

fn host() -> &'static Host {
    HOST.get_or_init(|| {
        let (tx, rx) = channel();
        let ticker = tx.clone();
        std::thread::Builder::new().name("plugins-host".into()).spawn(move || run(rx, ticker)).expect("the plugins' host thread");
        Host { tx, loaded: Mutex::new(BTreeMap::new()) }
    })
}

/// Load every enabled plugin's host half (the app at start, and after an install or a switch).
pub fn sync() {
    let want: BTreeMap<String, PathBuf> = plugins::enabled()
        .into_iter()
        .filter_map(|p| {
            let dir = plugins::plugin_dir(&p);
            let m = plugins::read_manifest(&dir).ok()?;
            m.main.as_ref().map(|_| (p.id.clone(), dir))
        })
        .collect();
    let h = host();
    let have: Vec<String> = h.loaded.lock().map(|l| l.keys().cloned().collect()).unwrap_or_default();
    for id in have.iter().filter(|id| !want.contains_key(*id)) {
        let _ = h.tx.send(Msg::Unload(id.clone()));
    }
    for (id, dir) in want {
        // Reloaded every time: an update changes the files under the same id.
        let _ = h.tx.send(Msg::Load(id, dir));
    }
}

/// Each loaded plugin and whether it loaded (or why not).
pub fn status() -> BTreeMap<String, Result<(), String>> {
    HOST.get().and_then(|h| h.loaded.lock().ok().map(|l| l.clone())).unwrap_or_default()
}

/// Run a plugin's command (`ctx.command`) with JSON `args`; its answer, or why there is none.
pub fn call(plugin: &str, command: &str, args: &Value) -> Result<Value> {
    call_within(plugin, command, args, CALL_TIMEOUT)
}

/// The same, waiting up to `timeout` (a terminal waiting for a run).
pub fn call_within(plugin: &str, command: &str, args: &Value, timeout: Duration) -> Result<Value> {
    let h = host();
    if !h.loaded.lock().map(|l| l.contains_key(plugin)).unwrap_or(false) {
        sync();
    }
    let (tx, rx) = channel();
    h.tx.send(Msg::Call { plugin: plugin.into(), command: command.into(), args: args.to_string(), reply: tx }).map_err(|_| anyhow!("the plugins' host stopped"))?;
    match rx.recv_timeout(timeout) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => bail!("{e}"),
        Err(_) => bail!("{plugin} didn't answer {command} within {} s", timeout.as_secs()),
    }
}

/// Tell every loaded plugin about something that happened in Echo (`ctx.on(name, fn)`).
#[allow(dead_code)]
pub fn event(name: &str, payload: &Value) {
    if let Some(h) = HOST.get() {
        let _ = h.tx.send(Msg::Event(name.into(), payload.to_string()));
    }
}

// ── the host thread ──

struct Plugin {
    ctx: Context,
    calls: HashMap<u64, Sender<Result<Value, String>>>,
}

/// The id of the plugin whose JavaScript runs now (what `__host` acts for) and its calls' replies.
#[derive(Default)]
struct Current {
    id: String,
    replies: Vec<(u64, Result<Value, String>)>,
}

fn run(rx: Receiver<Msg>, tx: Sender<Msg>) {
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return eprintln!("plugins: QuickJS can't start: {e}"),
    };
    rt.set_memory_limit(64 << 20);
    let started = Arc::new(Mutex::new(None::<Instant>));
    let stop = Arc::new(AtomicBool::new(false));
    {
        let started = started.clone();
        let stop = stop.clone();
        rt.set_interrupt_handler(Some(Box::new(move || stop.load(Ordering::Relaxed) || started.lock().ok().and_then(|s| *s).is_some_and(|t| t.elapsed() > SLICE))));
    }
    let current = Arc::new(Mutex::new(Current::default()));
    let mut plugins: HashMap<String, Plugin> = HashMap::new();
    let tick_every = Duration::from_secs(1);
    let mut last_tick = Instant::now();
    loop {
        let msg = rx.recv_timeout(tick_every.saturating_sub(last_tick.elapsed()));
        let mut slice = |f: &mut dyn FnMut()| {
            *started.lock().unwrap() = Some(Instant::now());
            f();
            // Promises and async functions go on until nothing is left to do now.
            loop {
                match rt.execute_pending_job() {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => {
                        eprintln!("plugins: {e}");
                        continue;
                    }
                }
            }
            *started.lock().unwrap() = None;
        };
        match msg {
            Ok(Msg::Load(id, dir)) => {
                plugins.remove(&id);
                let outcome = load(&rt, &id, &dir, &current, &tx, &mut slice);
                if let Ok(mut l) = host().loaded.lock() {
                    l.insert(id.clone(), outcome.as_ref().map(|_| ()).map_err(|e| e.to_string()));
                }
                match outcome {
                    Ok(ctx) => {
                        plugins.insert(id, Plugin { ctx, calls: HashMap::new() });
                    }
                    Err(e) => eprintln!("plugins: {id}: {e:#}"),
                }
            }
            Ok(Msg::Unload(id)) => {
                plugins.remove(&id);
                if let Ok(mut l) = host().loaded.lock() {
                    l.remove(&id);
                }
            }
            Ok(Msg::Call { plugin, command, args, reply }) => {
                let Some(p) = plugins.get_mut(&plugin) else {
                    let why = status().get(&plugin).and_then(|r| r.clone().err()).unwrap_or_else(|| "it isn't installed and on".into());
                    let _ = reply.send(Err(format!("{plugin} isn't loaded: {why}")));
                    continue;
                };
                let n = NEXT.fetch_add(1, Ordering::Relaxed);
                p.calls.insert(n, reply);
                set_current(&current, &plugin);
                let ctx = p.ctx.clone();
                slice(&mut || {
                    ctx.with(|c| {
                        if let Ok(f) = c.globals().get::<_, Function>("__call") {
                            if let Err(e) = f.call::<_, ()>((n, command.clone(), args.clone())).catch(&c) {
                                current.lock().unwrap().replies.push((n, Err(e.to_string())));
                            }
                        }
                    })
                });
            }
            Ok(Msg::Event(name, payload)) => {
                for (id, p) in &plugins {
                    set_current(&current, id);
                    let ctx = p.ctx.clone();
                    slice(&mut || ctx.with(|c| drop(c.globals().get::<_, Function>("__event").and_then(|f| f.call::<_, ()>((name.clone(), payload.clone()))))));
                }
            }
            Ok(Msg::Settle(id, token, out)) => {
                if let Some(p) = plugins.get(&id) {
                    set_current(&current, &id);
                    let ctx = p.ctx.clone();
                    slice(&mut || ctx.with(|c| drop(c.globals().get::<_, Function>("__settle").and_then(|f| f.call::<_, ()>((token, out.clone()))))));
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if last_tick.elapsed() >= tick_every && TIMERS.load(Ordering::Relaxed) {
            last_tick = Instant::now();
            let now = chrono::Utc::now().timestamp_millis() as f64;
            for (id, p) in &plugins {
                set_current(&current, id);
                let ctx = p.ctx.clone();
                slice(&mut || ctx.with(|c| drop(c.globals().get::<_, Function>("__tick").and_then(|f| f.call::<_, ()>((now,))))));
            }
        }
        // Replies the JavaScript gave (`__host('reply')`) go to their callers.
        let replies = std::mem::take(&mut current.lock().unwrap().replies);
        for (n, r) in replies {
            for p in plugins.values_mut() {
                if let Some(tx) = p.calls.remove(&n) {
                    let _ = tx.send(r.clone());
                }
            }
        }
    }
}

fn set_current(current: &Mutex<Current>, id: &str) {
    if let Ok(mut c) = current.lock() {
        c.id = id.into();
    }
}

fn load(rt: &Runtime, id: &str, dir: &Path, current: &Arc<Mutex<Current>>, tx: &Sender<Msg>, slice: &mut dyn FnMut(&mut dyn FnMut())) -> Result<Context> {
    let m = plugins::read_manifest(dir)?;
    let main = m.main.ok_or_else(|| anyhow!("no host half"))?;
    let file = plugins::inside_plugin(dir, &main)?;
    let source = std::fs::read_to_string(&file).map_err(|e| anyhow!("{main}: {e}"))?;
    let ctx = Context::full(rt).map_err(|e| anyhow!("{e}"))?;
    set_current(current, id);
    let mut outcome: Result<()> = Ok(());
    {
        let current = current.clone();
        let tx = tx.clone();
        let id_s = id.to_string();
        let ctx2 = ctx.clone();
        slice(&mut || {
            outcome = ctx2.with(|c| -> Result<()> {
                let g = c.globals();
                let cur = current.clone();
                let host_fn = Function::new(c.clone(), move |op: String, arg: String| -> String { host_op(&cur, &op, &arg) }).map_err(|e| anyhow!("{e}"))?;
                g.set("__host", host_fn).map_err(|e| anyhow!("{e}"))?;
                let (id2, tx2) = (id_s.clone(), tx.clone());
                let async_fn = Function::new(c.clone(), move |op: String, arg: String| -> u64 { host_async(&id2, &tx2, op, arg) }).map_err(|e| anyhow!("{e}"))?;
                g.set("__hostAsync", async_fn).map_err(|e| anyhow!("{e}"))?;
                c.eval::<(), _>(PRELUDE).catch(&c).map_err(|e| anyhow!("prelude: {e}"))?;
                let module = Module::declare(c.clone(), main.clone(), source.clone()).catch(&c).map_err(|e| anyhow!("{main}: {e}"))?;
                let (module, promise) = module.eval().catch(&c).map_err(|e| anyhow!("{main}: {e}"))?;
                promise.finish::<()>().catch(&c).map_err(|e| anyhow!("{main}: {e}"))?;
                let apply: Function = module.get("apply").map_err(|_| anyhow!("{main} exports no apply(ctx)"))?;
                let pctx: rquickjs::Value = g.get("__ctx").map_err(|e| anyhow!("{e}"))?;
                apply.call::<_, rquickjs::Value>((pctx,)).catch(&c).map_err(|e| anyhow!("apply: {e}"))?;
                Ok(())
            });
        });
    }
    outcome?;
    Ok(ctx)
}

/// `__host(op, json)`: one of Echo's services, at once; `{value}` or `{error}`.
fn host_op(current: &Mutex<Current>, op: &str, arg: &str) -> String {
    let id = current.lock().map(|c| c.id.clone()).unwrap_or_default();
    let arg: Value = serde_json::from_str(arg).unwrap_or(Value::Null);
    if op == "reply" {
        let n = arg["call"].as_u64().unwrap_or(0);
        let r = match arg.get("error").and_then(Value::as_str) {
            Some(e) => Err(e.to_string()),
            None => Ok(arg["value"].clone()),
        };
        if let Ok(mut c) = current.lock() {
            c.replies.push((n, r));
        }
        return json!({"value": null}).to_string();
    }
    match service(&id, op, &arg) {
        Ok(v) => json!({ "value": v }).to_string(),
        Err(e) => json!({ "error": format!("{e:#}") }).to_string(),
    }
}

/// `__hostAsync(op, json)`: started on a worker thread; its outcome settles the plugin's Promise.
fn host_async(id: &str, tx: &Sender<Msg>, op: String, arg: String) -> u64 {
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    let (id, tx) = (id.to_string(), tx.clone());
    std::thread::spawn(move || {
        let arg: Value = serde_json::from_str(&arg).unwrap_or(Value::Null);
        let out = match service_async(&id, &op, &arg) {
            Ok(v) => json!({ "value": v }),
            Err(e) => json!({ "error": format!("{e:#}") }),
        };
        let _ = tx.send(Msg::Settle(id, token, out.to_string()));
    });
    token
}

/// Where the plugin is installed.
fn package_dir(id: &str) -> Result<PathBuf> {
    plugins::enabled().into_iter().find(|p| p.id == id).map(|p| plugins::plugin_dir(&p)).ok_or_else(|| anyhow!("{id} isn't installed and on"))
}

fn package_file(id: &str, rel: &str) -> Result<PathBuf> {
    plugins::inside_plugin(&package_dir(id)?, rel)
}

/// A path in the plugin's own folder of the data folder: relative, no `..`, no links followed out.
fn store_path(id: &str, rel: &str) -> Result<PathBuf> {
    let base = plugins::data_dir(id);
    let rel = rel.trim_start_matches("./");
    let p = Path::new(rel);
    if p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        bail!("“{rel}” isn't a path inside the plugin's folder");
    }
    Ok(base.join(p))
}

fn service(id: &str, op: &str, arg: &Value) -> Result<Value> {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    Ok(match op {
        "id" => json!(id),
        "log" => {
            eprintln!("plugin {id}: [{}] {}", s(&arg[0]), s(&arg[1]));
            Value::Null
        }
        "store.read" => match std::fs::read_to_string(store_path(id, &s(arg))?) {
            Ok(t) => json!(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
            Err(e) => return Err(e.into()),
        },
        "store.write" => {
            let p = store_path(id, &s(&arg[0]))?;
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            paths::write_atomic(&p, s(&arg[1]).as_bytes())?;
            Value::Null
        }
        "store.list" => {
            let p = if s(arg).is_empty() { plugins::data_dir(id) } else { store_path(id, &s(arg))? };
            let mut names: Vec<String> = std::fs::read_dir(&p).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
            names.sort();
            json!(names)
        }
        "store.remove" => {
            let p = store_path(id, &s(arg))?;
            if p.is_dir() {
                std::fs::remove_dir_all(&p)?;
            } else if p.exists() {
                std::fs::remove_file(&p)?;
            }
            Value::Null
        }
        "store.path" => json!(if s(arg).is_empty() { plugins::data_dir(id) } else { store_path(id, &s(arg))? }.display().to_string()),
        "settings.get" => plugins::setting(id, &s(arg)).unwrap_or(Value::Null),
        // The plugin's own files as installed (templates, prompts), read-only.
        "package.read" => match package_file(id, &s(arg)) {
            Ok(f) => json!(std::fs::read_to_string(f)?),
            Err(_) => Value::Null,
        },
        "package.list" => {
            let dir = package_dir(id)?;
            let sub = s(arg);
            let d = if sub.is_empty() { dir } else { plugins::inside_plugin_dir(&dir, &sub)? };
            let mut names: Vec<String> = std::fs::read_dir(&d).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
            names.sort();
            json!(names)
        }
        "markdown" => json!(crate::minutes::markdown_html(&s(arg))),
        "notify" => {
            crate::app::notify(&s(&arg[0]), &s(&arg[1]));
            Value::Null
        }
        "inbox.set" => {
            inbox_set(id, &s(&arg[0]), Some(&arg[1]))?;
            Value::Null
        }
        "inbox.clear" => {
            inbox_set(id, &s(arg), None)?;
            Value::Null
        }
        "profiles" => {
            let set = crate::ai::Settings::load()?;
            json!({"default": set.default_id(), "profiles": set.all().into_iter().map(|p| json!({"id": p.id, "name": p.name})).collect::<Vec<_>>()})
        }
        "deliver.status" => crate::deliver::status(),
        "sessions.channel" => {
            // Only the channel the manifest names (the install sheet said which).
            let m = plugins::read_manifest(&package_dir(id)?)?;
            let allowed = m.uses.get("channel").and_then(|c| c["plugin"].as_str()).map(str::to_owned);
            let spec = arg["plugin"].as_str().unwrap_or("");
            if allowed.as_deref() != Some(spec) {
                bail!("{id} may listen only to the channel its manifest names ({})", allowed.unwrap_or_else(|| "none".into()));
            }
            let mut env: Vec<(String, String)> = vec![];
            for (k, v) in arg["env"].as_object().cloned().unwrap_or_default() {
                if !k.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') || k.starts_with("ANTHROPIC") || k.starts_with("CLAUDE") {
                    bail!("“{k}” can't be set for a channel");
                }
                env.push((k, v.as_str().unwrap_or("").to_string()));
            }
            if arg["token"] == "telegram" {
                let t = crate::deliver::load().telegram.ok_or_else(|| anyhow!("no Telegram bot yet: add yours in Settings → Plugins"))?;
                env.push(("TELEGRAM_BOT_TOKEN".into(), t.token));
            }
            CHANNELS.lock().map_err(|_| anyhow!("busy"))?.insert(id.into(), Channel { spec: spec.into(), env });
            Value::Null
        }
        "sessions.clear_channel" => {
            CHANNELS.lock().map_err(|_| anyhow!("busy"))?.remove(id);
            Value::Null
        }
        "system.which" => {
            let name = s(arg);
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                bail!("“{name}” isn't a program name");
            }
            let p = crate::app::find_program(&name);
            if Path::new(&p).is_file() { json!(p) } else { Value::Null }
        }
        o => bail!("Echo has no service “{o}”"),
    })
}

fn service_async(id: &str, op: &str, arg: &Value) -> Result<Value> {
    match op {
        // A Claude Code run on one of the user's AI profiles: Echo's guarded run (`claudejob.rs`).
        "claude.run" => {
            let spec = crate::claudejob::Spec::from_value(arg, &plugins::data_dir(id))?;
            let (answer, model, usage) = crate::claudejob::run(&spec)?;
            Ok(json!({"answer": answer, "model": model, "usage": usage}))
        }
        "claude.has_plugin" | "claude.install_plugin" => {
            let spec = arg[0].as_str().unwrap_or("");
            if !spec.contains('@') || !spec.bytes().all(|b| b.is_ascii_alphanumeric() || b"@._-".contains(&b)) {
                bail!("“{spec}” isn't a Claude Code plugin (name@marketplace)");
            }
            let profile = Some(arg[1].as_str().unwrap_or("")).filter(|p| !p.is_empty()).map(str::to_owned).unwrap_or_else(crate::ai::default_id);
            let ready = crate::ai::ready(&profile)?;
            let args: Vec<String> = if op == "claude.has_plugin" { vec!["plugin".into(), "list".into(), "--json".into()] } else { vec!["plugin".into(), "install".into(), spec.into(), "--scope".into(), "user".into()] };
            let out = crate::ai::claude_command(ready.profile(), &args, &paths::home(), false)?.wait_with_output()?;
            let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            if op == "claude.has_plugin" {
                // On and seen from the data folder (where Echo's sessions run): user scope, or this folder's project.
                let list: Vec<Value> = serde_json::from_slice(&out.stdout).map_err(|_| anyhow!("Claude Code's plugin list can't be read"))?;
                let home = dunce::canonicalize(paths::home()).unwrap_or_else(|_| paths::home());
                let usable = list.iter().any(|x| {
                    x["id"] == spec
                        && x["enabled"] == true
                        && (x["scope"] == "user" || x["projectPath"].as_str().and_then(|p| dunce::canonicalize(p).ok()).is_some_and(|p| p == home))
                });
                return Ok(json!(usable));
            }
            if !out.status.success() {
                bail!("Claude Code couldn't install {spec}: {}", text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no reason given").trim());
            }
            Ok(Value::Null)
        }
        "deliver.send" => {
            let kind = arg[0].as_str().unwrap_or("");
            let m = &arg[1];
            let s = |k: &str| m[k].as_str().map(str::to_owned);
            let to: Vec<String> = m["mail_to"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
            let (title, markdown, about, ai) = (s("title").unwrap_or_default(), s("markdown").unwrap_or_default(), s("about"), s("ai"));
            crate::deliver::ready(kind)?;
            crate::deliver::send(kind, &crate::deliver::Msg { title: &title, markdown: &markdown, about: about.as_deref(), ai: ai.as_deref(), mail_to: &to })?;
            Ok(Value::Null)
        }
        o => bail!("Echo has no service “{o}”"),
    }
}

// ── the plugins' Inbox items ──

fn inbox_file(id: &str) -> PathBuf {
    plugins::data_dir(id).join("inbox.json")
}

fn inbox_set(id: &str, key: &str, item: Option<&Value>) -> Result<()> {
    if key.is_empty() || key.len() > 80 {
        bail!("an Inbox item needs a short key");
    }
    let f = inbox_file(id);
    let mut all: serde_json::Map<String, Value> = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    match item {
        Some(v) => {
            all.insert(key.into(), v.clone());
        }
        None => {
            all.remove(key);
        }
    }
    std::fs::create_dir_all(plugins::data_dir(id))?;
    paths::write_atomic(&f, serde_json::to_string_pretty(&all)?.as_bytes())?;
    Ok(())
}

/// Every enabled plugin's open Inbox items: `(plugin, key, item)`.
pub fn inbox_items() -> Vec<(String, String, Value)> {
    plugins::enabled()
        .into_iter()
        .flat_map(|p| {
            let all: serde_json::Map<String, Value> = std::fs::read(inbox_file(&p.id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            all.into_iter().map(move |(k, v)| (p.id.clone(), k, v)).collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_half_gets_echos_services_and_answers_commands() {
        let (_env, root) = paths::testenv::folders("jshost");
        let mk = root.join("market");
        let d = mk.join("plugins/hello");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "hello", "name": "Hello", "version": "0.1.0", "main": "main.js"}).to_string()).unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              let ticks = 0
              ctx.every(10, () => { ticks++ })
              ctx.command('greet', (a) => `hello ${a.name} from ${ctx.id}`)
              ctx.command('save', (a) => { ctx.store.write('notes/a.txt', a.text); return ctx.store.list('notes') })
              ctx.command('read', () => ctx.store.read('notes/a.txt'))
              ctx.command('later', async () => { await null; return 42 })
              ctx.command('escape', () => ctx.store.read('../../config.json'))
              ctx.command('boom', () => { throw new Error('nope') })
              ctx.command('md', () => ctx.markdown('**hi** <b>'))
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "hello", "source": "./plugins/hello"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review("hello", Some(&name)).unwrap().uses;
        plugins::install("hello", Some(&name), &agreed).unwrap();
        sync();
        let greet = (0..50).find_map(|_| call("hello", "greet", &json!({"name": "Ann"})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(greet.unwrap(), "hello Ann from hello");
        assert_eq!(call("hello", "save", &json!({"text": "x"})).unwrap(), json!(["a.txt"]));
        assert_eq!(call("hello", "read", &json!({})).unwrap(), "x");
        assert_eq!(call("hello", "later", &json!({})).unwrap(), 42);
        assert!(call("hello", "escape", &json!({})).unwrap_err().to_string().contains("inside the plugin"));
        assert_eq!(call("hello", "boom", &json!({})).unwrap_err().to_string(), "nope");
        assert!(call("hello", "nothing", &json!({})).is_err());
        assert_eq!(call("hello", "md", &json!({})).unwrap(), crate::minutes::markdown_html("**hi** <b>"));
        assert!(plugins::data_dir("hello").join("notes/a.txt").is_file());
        plugins::remove("hello", false).unwrap();
        sync();
    }
}
