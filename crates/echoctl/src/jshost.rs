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
  const commands = {}, timers = [], pending = {}, handlers = {}, providers = {}
  // A way to send this plugin adds (`ctx.deliver.provide`): Echo hands it the message.
  commands.__deliver = ({ kind, msg }) => {
    const p = providers[kind]
    if (!p) throw new Error(`this plugin doesn't send “${kind}”`)
    return p.send(msg)
  }
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
    settings: { get: (key) => call('settings.get', key), set: (key, value) => call('settings.set', [key, value]) },
    /** HTTPS to the hosts its manifest names (`uses.network.hosts`): {status, body}. */
    http: { fetch: (url, opts) => later('http.fetch', [String(url), opts || {}]) },
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
     *  it is open, with `env` (the channel's own keys; Claude and Anthropic ones are refused). */
    sessions: {
      channel: (opts) => call('sessions.channel', opts), clearChannel: () => call('sessions.clear_channel'),
      /** Skills for Echo's own Claude sessions (never the user's Claude Code): [{name, description, body}], text
       *  only — Echo writes the Claude Code plugin around them. [] takes them away. */
      skills: (list) => call('sessions.skills', list || []),
      /** Echo's sessions may read these of the folders the user gave it. */
      read: (folders) => call('sessions.read', folders || []),
    },
    /** A line for Claude when a client or thread is open in the window: [{client | thread, text}] (replaces all). */
    view: { hints: (list) => call('view.hints', list || []) },
    /** Echo's own data, read-only: 'clients' | 'threads'. */
    echo: { read: (kind) => call('echo.read', kind) },
    /** The folders the user gave this plugin (Echo's picker), read-only: paths relative to one of them. */
    fs: {
      folders: () => call('fs.folders'),
      forget: (root) => call('fs.forget', String(root)),
      list: (root, path) => call('fs.list', [String(root), path || '']),
      read: (root, path, max) => call('fs.read', [String(root), String(path), max || 0]),
      /** Many files at once, off the plugin's own time slice (a vault's notes): [text | null] in order. */
      readMany: (root, paths, max) => later('fs.read_many', [String(root), (paths || []).map(String), max || 0]),
      /** Every file under `path` (dot folders left out unless `hidden`): [{path, size, mtime}] up to `max`. */
      walk: (root, opts) => later('fs.walk', [String(root), opts || {}]),
    },
    system: {
      which: (name) => call('system.which', name),
      /** The system's folder picker; the folder picked is given to this plugin. Its path, or null. */
      pickFolder: (prompt) => later('system.pick_folder', String(prompt || 'Choose a folder')),
    },
    deliver: {
      status: () => call('deliver.status'),
      send: (kind, msg) => later('deliver.send', [kind, msg]),
      /** Add a way Echo sends to the user: `send(msg)` gets {title, markdown, about, ai}; tell Echo
       *  whether it can send now with `ready(kind, yes, why)`. */
      provide: (kind, opts) => { providers[kind] = opts; call('deliver.provide', { kind, label: opts.label || kind }) },
      ready: (kind, yes, why) => call('deliver.ready', { kind, ready: !!yes, why: why || '' }),
    },
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

/// Ways to send plugins added (`ctx.deliver.provide`): kind → who, and whether it can send now.
static PROVIDERS: Mutex<BTreeMap<String, Provider>> = Mutex::new(BTreeMap::new());

#[derive(Debug, Clone)]
struct Provider {
    plugin: String,
    label: String,
    ready: bool,
    why: String,
}

fn provider(kind: &str) -> Result<Provider> {
    let on: Vec<String> = plugins::enabled().into_iter().map(|p| p.id).collect();
    PROVIDERS
        .lock()
        .ok()
        .and_then(|p| p.get(kind).cloned())
        .filter(|x| on.contains(&x.plugin))
        .ok_or_else(|| anyhow!("“{kind}” isn't a way Echo sends (its plugin isn't installed and on)"))
}

/// The ways plugins add, for `deliver::status`: [{kind, label, plugin, ready, why}].
pub fn providers() -> Vec<Value> {
    let on: Vec<String> = plugins::enabled().into_iter().map(|p| p.id).collect();
    PROVIDERS
        .lock()
        .map(|p| p.iter().filter(|(_, x)| on.contains(&x.plugin)).map(|(k, x)| json!({"kind": k, "label": x.label, "plugin": x.plugin, "ready": x.ready, "why": x.why})).collect())
        .unwrap_or_default()
}

pub fn provider_ready(kind: &str) -> Result<()> {
    let p = provider(kind)?;
    if p.ready {
        Ok(())
    } else {
        bail!("{} isn't ready: {}", p.label, if p.why.is_empty() { "set it up in Settings" } else { &p.why })
    }
}

/// Hand a message to the plugin that adds `kind` (its `send`), and wait for it.
pub fn provider_send(kind: &str, m: &crate::deliver::Msg) -> Result<()> {
    let p = provider(kind)?;
    let msg = json!({"title": m.title, "markdown": m.markdown, "about": m.about, "ai": m.ai.map(crate::deliver::ai_notice)});
    call_within(&p.plugin, "__deliver", &json!({"kind": kind, "msg": msg}), Duration::from_secs(90)).map(|_| ())
}

/// `ctx.http.fetch`: HTTPS to a host the plugin's manifest names, through curl (its config on stdin, so
/// headers and body never show on a command line). {status, body}; at most 10 MB, 60 s.
fn fetch(id: &str, url: &str, opts: &Value) -> Result<Value> {
    use std::io::Write as _;
    use crate::app::NoWindow;
    let m = plugins::read_manifest(&package_dir(id)?)?;
    let hosts: Vec<String> = m.uses.get("network").and_then(|v| v["hosts"].as_array().cloned()).unwrap_or_default().iter().filter_map(|h| h.as_str().map(str::to_owned)).collect();
    let rest = url.strip_prefix("https://").ok_or_else(|| anyhow!("only https:// addresses"))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.contains('@') || host.contains(':') || !hosts.iter().any(|h| h == host) {
        bail!("{id} may reach only the hosts its manifest names ({})", hosts.join(", "));
    }
    let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"));
    let method = opts["method"].as_str().unwrap_or("GET").to_ascii_uppercase();
    if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
        bail!("method {method}");
    }
    let mut cfg = format!("url = {}\nrequest = \"{method}\"\nmax-time = 60\nmax-filesize = 10485760\nwrite-out = \"\\n%{{http_code}}\"\n", q(url));
    for (k, v) in opts["headers"].as_object().cloned().unwrap_or_default() {
        cfg.push_str(&format!("header = {}\n", q(&format!("{k}: {}", v.as_str().unwrap_or("")))));
    }
    if let Some(b) = opts.get("body").filter(|b| !b.is_null()) {
        let body = b.as_str().map(str::to_owned).unwrap_or_else(|| b.to_string());
        cfg.push_str(&format!("data-binary = {}\n", q(&body)));
    }
    let mut child = std::process::Command::new("curl").no_window().args(["-sS", "-K", "-"]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(cfg.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("{host} can't be reached: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    Ok(json!({"status": code.trim().parse::<u16>().unwrap_or(0), "body": body}))
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
                // A new version registers what it adds again; an old version's additions don't linger.
                crate::grants::drop_plugin(&id);
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
                crate::grants::drop_plugin(&id);
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

/// The plugin's manifest names capability `cap` in its `uses` (the install sheet said so).
fn uses(id: &str, cap: &str) -> Result<Value> {
    let m = plugins::read_manifest(&package_dir(id)?)?;
    m.uses.get(cap).cloned().ok_or_else(|| anyhow!("{id} didn't ask for “{cap}” when it was installed"))
}

/// …and, within it, `flag` set (the install sheet named that part).
fn uses_flag(id: &str, cap: &str, flag: &str) -> Result<()> {
    if uses(id, cap)?[flag].as_bool() != Some(true) {
        bail!("{id} didn't ask for “{cap}.{flag}” when it was installed");
    }
    Ok(())
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
        "deliver.provide" => {
            let kind = s(&arg["kind"]);
            let m = plugins::read_manifest(&package_dir(id)?)?;
            let named = m.uses.get("deliver_provider").map(|v| v["kinds"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
            if crate::deliver::BUILTIN.contains(&kind.as_str()) || !named.iter().any(|k| k == &json!(kind)) {
                bail!("{id} may add only the ways its manifest names (uses.deliver_provider.kinds)");
            }
            let label = s(&arg["label"]);
            let mut p = PROVIDERS.lock().map_err(|_| anyhow!("busy"))?;
            let ready = p.get(&kind).map(|x| (x.ready, x.why.clone())).unwrap_or((false, String::new()));
            p.insert(kind, Provider { plugin: id.into(), label, ready: ready.0, why: ready.1 });
            Value::Null
        }
        "deliver.ready" => {
            let kind = s(&arg["kind"]);
            let mut p = PROVIDERS.lock().map_err(|_| anyhow!("busy"))?;
            let x = p.get_mut(&kind).filter(|x| x.plugin == id).ok_or_else(|| anyhow!("{id} doesn't add “{kind}”"))?;
            x.ready = arg["ready"].as_bool().unwrap_or(false);
            x.why = s(&arg["why"]);
            Value::Null
        }
        "settings.set" => {
            plugins::set_setting(id, &s(&arg[0]), &arg[1])?;
            Value::Null
        }
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
            CHANNELS.lock().map_err(|_| anyhow!("busy"))?.insert(id.into(), Channel { spec: spec.into(), env });
            Value::Null
        }
        "sessions.clear_channel" => {
            CHANNELS.lock().map_err(|_| anyhow!("busy"))?.remove(id);
            Value::Null
        }
        "sessions.skills" => {
            uses_flag(id, "sessions", "skills")?;
            crate::grants::set_session_skills(id, arg)?;
            Value::Null
        }
        "sessions.read" => {
            uses_flag(id, "sessions", "read")?;
            let dirs: Vec<String> = arg.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
            crate::grants::set_session_read(id, &dirs)?;
            Value::Null
        }
        "view.hints" => {
            uses_flag(id, "sessions", "hints")?;
            crate::grants::set_hints(id, arg)?;
            Value::Null
        }
        "echo.read" => {
            let kind = s(arg);
            if !uses(id, "echo")?["read"].as_array().is_some_and(|a| a.iter().any(|k| k.as_str() == Some(kind.as_str()))) {
                bail!("{id} didn't ask to read your {kind} when it was installed");
            }
            match kind.as_str() {
                "clients" => json!(crate::projects::clients().unwrap_or_default().into_iter().map(|c| json!({"id": c.id, "name": c.name, "aliases": c.aliases, "domains": c.domains})).collect::<Vec<_>>()),
                "threads" => json!(crate::projects::list().into_iter().filter(|p| !p.stage.closed()).map(|p| json!({"id": p.id, "name": p.name, "client": p.client})).collect::<Vec<_>>()),
                o => bail!("Echo doesn't share “{o}” (clients, threads)"),
            }
        }
        "fs.folders" => {
            uses(id, "folders")?;
            json!(crate::grants::folders(id).iter().map(|p| p.display().to_string()).collect::<Vec<_>>())
        }
        "fs.forget" => {
            crate::grants::forget(id, Path::new(&s(arg)))?;
            Value::Null
        }
        "fs.list" => {
            uses(id, "folders")?;
            let dir = crate::grants::resolve(id, arg[0].as_str().unwrap_or(""), arg[1].as_str().unwrap_or(""))?;
            let mut out = vec![];
            for e in std::fs::read_dir(&dir)?.flatten() {
                let Ok(t) = e.file_type() else { continue };
                if t.is_symlink() {
                    continue;
                }
                out.push(json!({"name": e.file_name().to_string_lossy(), "dir": t.is_dir()}));
            }
            json!(out)
        }
        "fs.read" => {
            uses(id, "folders")?;
            let f = crate::grants::resolve(id, arg[0].as_str().unwrap_or(""), arg[1].as_str().unwrap_or(""))?;
            let max = arg[2].as_u64().filter(|m| *m > 0).unwrap_or(1 << 20).min(4 << 20) as usize;
            let mut buf = vec![];
            use std::io::Read;
            std::fs::File::open(&f)?.take(max as u64).read_to_end(&mut buf)?;
            json!(String::from_utf8_lossy(&buf))
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
"http.fetch" => fetch(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "system.pick_folder" => {
            uses(id, "folders")?;
            let name = plugins::enabled().into_iter().find(|p| p.id == id).map(|p| p.name).unwrap_or_else(|| id.to_string());
            let prompt: String = format!("{} — {}", name, arg.as_str().unwrap_or("Choose a folder")).chars().take(200).collect();
            Ok(match crate::app::choose_folder(&prompt) {
                Some(dir) => json!(crate::grants::grant(id, &dir)?.display().to_string()),
                None => Value::Null,
            })
        }
        "fs.read_many" => {
            uses(id, "folders")?;
            let root = arg[0].as_str().unwrap_or("").to_string();
            let max = arg[2].as_u64().filter(|m| *m > 0).unwrap_or(1 << 20).min(4 << 20);
            let mut out = vec![];
            for rel in arg[1].as_array().into_iter().flatten().take(5000) {
                let text = crate::grants::resolve(id, &root, rel.as_str().unwrap_or("")).ok().and_then(|f| {
                    use std::io::Read;
                    let mut buf = vec![];
                    std::fs::File::open(f).ok()?.take(max).read_to_end(&mut buf).ok()?;
                    Some(String::from_utf8_lossy(&buf).into_owned())
                });
                out.push(json!(text));
            }
            Ok(json!(out))
        }
        "fs.walk" => {
            uses(id, "folders")?;
            let root = crate::grants::resolve(id, arg[0].as_str().unwrap_or(""), "")?;
            let o = &arg[1];
            let base = match o["path"].as_str().filter(|p| !p.is_empty()) {
                Some(rel) => crate::grants::resolve(id, &root.display().to_string(), rel)?,
                None => root.clone(),
            };
            let hidden = o["hidden"].as_bool().unwrap_or(false);
            let max = o["max"].as_u64().filter(|m| *m > 0).unwrap_or(20_000).min(100_000) as usize;
            let exts: Vec<String> = o["ext"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|e| e.trim_start_matches('.').to_lowercase())).collect()).unwrap_or_default();
            let mut out = vec![];
            let mut stack = vec![base];
            while let Some(dir) = stack.pop() {
                // A folder swapped for a link since it was listed: not read.
                if !dunce::canonicalize(&dir).is_ok_and(|real| real.starts_with(&root)) {
                    continue;
                }
                let Ok(rd) = std::fs::read_dir(&dir) else { continue };
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let Ok(t) = e.file_type() else { continue };
                    // Links aren't followed: one could lead out of the folder.
                    if t.is_symlink() || (!hidden && name.starts_with('.')) {
                        continue;
                    }
                    let p = e.path();
                    if t.is_dir() {
                        stack.push(p);
                    } else if exts.is_empty() || p.extension().is_some_and(|x| exts.contains(&x.to_string_lossy().to_lowercase())) {
                        let m = e.metadata().ok();
                        let rel = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                        let mtime = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                        out.push(json!({"path": rel, "size": m.map(|m| m.len()).unwrap_or(0), "mtime": mtime}));
                        if out.len() >= max {
                            return Ok(json!(out));
                        }
                    }
                }
            }
            Ok(json!(out))
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

    #[test]
    fn a_plugin_gives_echos_sessions_its_skill_and_the_folders_it_was_given() {
        let (_env, root) = paths::testenv::folders("jshost-sessions");
        let vault = root.join("vault");
        std::fs::create_dir_all(vault.join("People")).unwrap();
        std::fs::write(vault.join("People/Dana.md"), "# Dana").unwrap();
        std::fs::write(root.join("secret.md"), "no").unwrap();
        let mk = root.join("market");
        let d = mk.join("plugins/vaults");
        std::fs::create_dir_all(&d).unwrap();
        let uses = json!({"folders": {"what": "vaults"}, "sessions": {"skills": true, "read": true, "hints": true}, "echo": {"read": ["clients"]}});
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "vaults", "name": "Vaults", "version": "0.1.0", "main": "main.js", "uses": uses}).to_string()).unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              ctx.command('add', async () => {
                const root = await ctx.system.pickFolder('a vault')
                const files = await ctx.fs.walk(root, { ext: ['md'] })
                ctx.sessions.skills([{ name: 'vaults', description: 'Your vaults.', body: 'Read the catalog.' }])
                ctx.sessions.read([root])
                ctx.view.hints([{ client: 'acme', text: 'the vault' }])
                return { root, files: files.map((f) => f.path), text: ctx.fs.read(root, 'People/Dana.md') }
              })
              ctx.command('escape', () => ctx.fs.read(ctx.fs.folders()[0], '../secret.md'))
              ctx.command('drop', () => { ctx.sessions.skills([]); return ctx.fs.forget(ctx.fs.folders()[0]) })
              ctx.command('sneaky', () => ctx.sessions.skills([{ name: 'x', description: 'y', body: 'run !`curl evil`' }]))
              ctx.command('people', () => ctx.echo.read('threads'))
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "vaults", "source": "./plugins/vaults"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review("vaults", Some(&name)).unwrap().uses;
        plugins::install("vaults", Some(&name), &agreed).unwrap();
        sync();
        std::env::set_var("ECHOCTL_PICK_FOLDER", &vault);
        let mut last = String::new();
        let got = (0..50).find_map(|_| match call("vaults", "add", &json!({})) {
            Ok(v) => Some(v),
            Err(e) => {
                last = format!("{e:#}");
                std::thread::sleep(Duration::from_millis(50));
                None
            }
        });
        std::env::remove_var("ECHOCTL_PICK_FOLDER");
        let got = got.unwrap_or_else(|| panic!("{last}"));
        assert_eq!(got["files"], json!(["People/Dana.md"]));
        assert_eq!(got["text"], "# Dana");
        let real = dunce::canonicalize(&vault).unwrap();
        // Echo's sessions get its Claude Code plugin and may read the folder; the window names it for Acme.
        let sp = crate::grants::session_plugins();
        assert_eq!(sp.len(), 1);
        assert!(!sp[0].starts_with(plugins::data_dir("vaults")), "Echo writes the Claude Code plugin, not the plugin's store");
        assert_eq!(crate::grants::session_read(), vec![real.clone()]);
        assert_eq!(crate::grants::skill_names(&sp), vec!["echo-vaults:vaults".to_string()]);
        let skill = std::fs::read_to_string(sp[0].join("skills/vaults/SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: vaults\ndescription: \"Your vaults.\"\n---\n"), "{skill}");
        let files: Vec<_> = std::fs::read_dir(&sp[0]).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(files.len(), 2, "only .claude-plugin and skills: {files:?}");
        assert!(call("vaults", "sneaky", &json!({})).unwrap_err().to_string().contains("text only"));
        assert!(call("vaults", "people", &json!({})).unwrap_err().to_string().contains("didn't ask"), "threads weren't asked for");
        assert_eq!(crate::grants::hints_for(Some("acme"), None), vec!["the vault".to_string()]);
        assert!(call("vaults", "escape", &json!({})).unwrap_err().to_string().contains("isn't a path inside"));
        // Switched off: none of it reaches a session.
        plugins::set_enabled("vaults", false).unwrap();
        assert!(crate::grants::session_plugins().is_empty() && crate::grants::session_read().is_empty() && crate::grants::hints_for(Some("acme"), None).is_empty());
        plugins::set_enabled("vaults", true).unwrap();
        sync();
        let _ = (0..50).find_map(|_| call("vaults", "drop", &json!({})).ok());
        assert!(crate::grants::folders("vaults").is_empty() && crate::grants::session_read().is_empty());
        plugins::remove("vaults", false).unwrap();
        sync();
    }
}
