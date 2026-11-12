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

mod test_hosts;
pub mod work;

use test_hosts::test_host;
#[cfg(test)]
use test_hosts::test_host_in;

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
    /** True in a terminal (`echoctl plugin call`): a command runs and the process ends — skip start-up work there. */
    terminal: call('terminal'),
    log: (...a) => call('log', ['info', a.map(String).join(' ')]),
    command: (name, fn) => { commands[name] = fn },
    every: (seconds, fn) => { timers.push({ every: Math.max(10, seconds) * 1000, next: 0, fn }) },
    /** Hear an event (`uses.events.on`): a promise, refused (and the handler dropped) for one not asked for. */
    on: (event, fn) => {
      try {
        call('events.on', String(event))
      } catch (e) {
        call('log', ['error', `ctx.on(${event}): ${e.message}`])
        const refused = Promise.reject(e)
        refused.catch(() => {}) // the plugin may not look: then it is only logged
        return refused
      }
      (handlers[event] = handlers[event] || []).push(fn)
      return Promise.resolve(true)
    },
    store: {
      read: (path) => call('store.read', path),
      write: (path, text) => call('store.write', [path, String(text)]),
      list: (dir) => call('store.list', dir || ''),
      remove: (path) => call('store.remove', path),
      path: (path) => call('store.path', path || ''),
      /** What it keeps in its store (`dir`: one folder of it): {bytes, files} — its own files only, not its settings. */
      size: (dir) => call('store.size', dir || ''),
    },
    /** Its `cache/` (Echo removes what is there over 7 days): `size()` {bytes, files}. */
    cache: { size: () => call('store.size', 'cache') },
    settings: { get: (key) => call('settings.get', key), set: (key, value) => call('settings.set', [key, value]) },
    /** HTTPS to the hosts its manifest names (`uses.network.hosts`): {status, body}. */
    http: { fetch: (url, opts) => later('http.fetch', [String(url), opts || {}]) },
    package: { read: (path) => call('package.read', path), list: (dir) => call('package.list', dir || '') },
    markdown: (md) => call('markdown', String(md)),
    notify: (title, body) => call('notify', [String(title), String(body || '')]),
    inbox: { set: (key, item) => call('inbox.set', [key, item]), clear: (key) => call('inbox.clear', key) },
    /** Today's widgets it declares (`widgets` in its manifest): `provide` answers Echo's ask ({size}) with the data,
     *  `set` gives it when the plugin learns something, `clear` takes it off Today until the next. */
    widgets: {
      provide: (id, fn) => { commands['__widget:' + id] = (a) => fn(a || {}) },
      set: (id, data) => call('widgets.set', [String(id), data === undefined ? null : data]),
      clear: (id) => call('widgets.clear', String(id)),
    },
    /** Its items on Today → My work and in threads' Work (uses.work): `provide(({scope, thread}) => rows)` answers
     *  Echo's ask — scope "me" (My work) or "thread" (that thread's Work tab) — with [{id, title, kind, where?, url?,
     *  due?, late?, thread?}] (200 at most); `changed()` has Echo ask again. */
    work: {
      provide: (fn) => { call('work.provide'); commands.__work = (a) => fn(a || {}) },
      changed: () => call('work.changed'),
    },
    /** Its own values per thread (no grant: its data, by thread id): `get(thread, key)` → value | null, `set(thread,
     *  key, value)` (JSON, 16 KB at most, 64 keys a thread, 4 MB in all; null removes it), `list(key)` → [{thread,
     *  value}] of the threads that have it. They follow the thread: gone when it is deleted (hear it: `threads_changed` in
     *  uses.events.on — {thread, born, change: renamed, name, from} | {…merged, into, dropped: its keys that didn't fit} |
     *  {…deleted, values: its own}; born: the thread's first stage time, another thread's if its id is used again). */
    threads: {
      get: (thread, key) => call('threads.get', [String(thread), String(key)]),
      set: (thread, key, value) => call('threads.set', [String(thread), String(key), value === undefined ? null : value]),
      list: (key) => call('threads.list', String(key)),
    },
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
    /** Echo's own data, read-only: 'clients' | 'threads' | 'people' | 'dictionary' | 'sources' (opts: kind, thread,
     *  since, limit) | 'source' (an id: call:<folder> | mail:<source>). Its events (`ctx.on`, uses.events.on —
     *  minutes_written and sources_changed come with reading sources): `plugins::EVENTS`. */
    echo: {
      read: (kind, arg) => call('echo.read', arg === undefined ? kind : [kind, arg]),
      /** Names for one thing into the user's dictionary (uses.echo.write: dictionary): {name, synonyms, spellings, kind, client}. */
      dictionary: { add: (term) => call('echo.dictionary.add', term) },
    },
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
      /** With `uses.folders.write`: write a file (atomically), move one (never over another), delete one or an empty
       *  folder — inside the folders the user gave it, never through a link. */
      write: (root, path, text) => call('fs.write', [String(root), String(path), String(text)]),
      move: (root, from, to) => call('fs.move', [String(root), String(from), String(to)]),
      remove: (root, path) => call('fs.remove', [String(root), String(path)]),
    },
    system: {
      which: (name) => call('system.which', name),
      /** Another app's file its manifest names in `uses.files.read` (that very entry, e.g. "~/…/obsidian.json"):
       *  its text (1 MB at most), or null when it isn't on this computer. */
      readFile: (entry) => call('files.read', String(entry)),
      /** The system's folder picker; the folder picked is given to this plugin. Its path, or null. */
      pickFolder: (prompt) => later('system.pick_folder', String(prompt || 'Choose a folder')),
    },
    /** The user's own sign-in to a host its manifest names in `uses.signin.hosts`, from the GitHub CLI or git on this
     *  computer: `get` resolves {token, from: 'gh' | 'git', user?} or null when there is none (kept a minute;
     *  `{fresh: true}` asks again, e.g. after the token was refused); `sources` says which of the two programs are
     *  here ({gh, git}), reading no sign-in. */
    signin: {
      get: (host, opts) => later('signin.get', [String(host), !!(opts && opts.fresh)]),
      sources: (host) => call('signin.sources', String(host)),
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
    Call { plugin: String, command: String, args: String, n: u64, reply: Sender<Result<Value, String>> },
    /// A call's caller stopped waiting (its timeout): its reply is dropped, never kept for a plugin that doesn't settle.
    Forget(String, u64),
    Event(String, String),
    /// An event whose payload differs per plugin: each one's fields set on the shared part.
    EventEach(String, Value, serde_json::Map<String, Value>),
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

/// `ctx.http.fetch`: HTTPS to a host the plugin's manifest names, or to an address the user listed in a setting its
/// manifest names in `network.hosts_from` (`plugins::addresses`: plain http too, inside the user's own network), through
/// curl (its config on stdin, so headers and body never show on a command line). {status, body}; at most 10 MB, 60 s.
/// With `save: <path>` (relative, in the plugin's store) a 2xx answer's bytes go to that file and the answer is {status,
/// body: "", saved, bytes}: the body is text, so audio and images can't come through it.
fn fetch(id: &str, url: &str, opts: &Value) -> Result<Value> {
    use std::io::Write as _;
    use crate::app::NoWindow;
    if opts["save"].as_str().is_some_and(|s| !s.is_empty()) || opts["method"].as_str().is_some_and(|m| !m.eq_ignore_ascii_case("GET")) {
        read_only_here("a request other than GET")?;
    }
    let m = plugins::read_manifest(&package_dir(id)?)?;
    let hosts: Vec<String> = m.uses.get("network").and_then(|v| v["hosts"].as_array().cloned()).unwrap_or_default().iter().filter_map(|h| h.as_str().map(str::to_owned)).collect();
    // What the user listed, each checked again as when it was saved (the file may have been edited since).
    let listed: Vec<String> = plugins::address_settings(&m.uses).iter().filter_map(|k| plugins::listed_addresses(id, k)).collect();
    let granted: Vec<plugins::addresses::Address> = listed.iter().filter_map(|v| plugins::addresses::parse_list(v).ok()).flatten().collect();
    let refuse = || anyhow!("{id} may reach only the hosts its manifest names ({}) and the addresses you list in its settings ({})", hosts.join(", "), listed.join(", "));
    let rest = url.split_once("://").map_or("", |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if !granted.iter().any(|a| a.matches(url)) {
        let named = url.strip_prefix("https://").map(|r| r.split(['/', '?', '#']).next().unwrap_or(""));
        if !named.is_some_and(|h| !h.contains('@') && !h.contains(':') && hosts.iter().any(|x| x == h)) {
            return Err(refuse());
        }
    }
    // Checked on the real host above; a development build may send it to a fake service instead (or refuse it).
    let fake = test_host(url, &hosts)?;
    let url = fake.as_deref().unwrap_or(url);
    /// The half-written download: its own name per fetch, gone however the fetch ends unless renamed into place.
    struct Part(PathBuf);
    impl Drop for Part {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let save = match opts["save"].as_str().filter(|s| !s.is_empty()) {
        Some(rel) => {
            let to = writable_store_path(id, rel)?;
            std::fs::create_dir_all(to.parent().unwrap_or(&to))?;
            let mut part = to.clone().into_os_string();
            part.push(format!(".{}-{}.part", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            Some((rel, to, Part(PathBuf::from(part))))
        }
        None => None,
    };
    let method = opts["method"].as_str().unwrap_or("GET").to_ascii_uppercase();
    if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
        bail!("method {method}");
    }
    // A NUL would end curl's config line early; a header name is a plain token (curl reads `@file` as a file).
    let headers = opts["headers"].as_object().cloned().unwrap_or_default();
    let body = opts.get("body").filter(|b| !b.is_null()).map(|b| b.as_str().map(str::to_owned).unwrap_or_else(|| b.to_string()));
    if url.contains('\0') || body.as_deref().is_some_and(|b| b.contains('\0')) || headers.iter().any(|(k, v)| k.is_empty() || !k.bytes().all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c)) || v.as_str().unwrap_or("").contains(['\0', '\n', '\r'])) {
        bail!("a URL, header or body with a NUL, a line break in a header or a header name that isn't a plain word");
    }
    let mut cfg = curl_head(url, &method, fake.is_some());
    if let Some((_, _, part)) = &save {
        cfg.push_str(&format!("output = {}\n", curl_quote(&part.0.to_string_lossy())));
    }
    for (k, v) in headers {
        cfg.push_str(&format!("header = {}\n", curl_quote(&format!("{k}: {}", v.as_str().unwrap_or("")))));
    }
    // `data-raw`: sent as written; `data-binary` would read a body starting with `@` as a file.
    if let Some(body) = body {
        cfg.push_str(&format!("data-raw = {}\n", curl_quote(&body)));
    }
    // `-q` first: the user's own ~/.curlrc (a `location` that follows redirects) isn't read.
    let mut child = std::process::Command::new("curl").no_window().args(["-q", "-sS", "-K", "-"]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(cfg.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("{host} can't be reached: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    let status = code.trim().parse::<u16>().unwrap_or(0);
    let Some((rel, to, part)) = save else { return Ok(json!({"status": status, "body": body})) };
    if (200..300).contains(&status) {
        // An empty answer may leave curl's file unmade.
        if !part.0.exists() {
            std::fs::write(&part.0, b"")?;
        }
        let bytes = std::fs::metadata(&part.0).map(|m| m.len()).unwrap_or(0);
        std::fs::rename(&part.0, &to)?;
        return Ok(json!({"status": status, "body": "", "saved": rel, "bytes": bytes}));
    }
    // Not a success: what the server said (text, cut), not a file.
    let said = std::fs::read(&part.0).map(|b| String::from_utf8_lossy(&b[..b.len().min(4096)]).into_owned()).unwrap_or_default();
    Ok(json!({"status": status, "body": said}))
}

/// Async operations and command calls in flight, numbered.
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Off in a terminal (`echoctl plugin call`): the plugins' timers are the app's alone.
static TIMERS: AtomicBool = AtomicBool::new(true);

/// This process runs commands only, never the plugins' timers.
pub fn no_timers() {
    TIMERS.store(false, Ordering::Relaxed);
}

/// This process is the app (`app.rs`, as it starts the plugins' host): only it hears Echo's events for the plugins
/// and takes what other processes queued for them. Any other process that loads a host half (`echoctl plugin call`,
/// `echoctl widget show`) is not, timers or not.
static APP: AtomicBool = AtomicBool::new(false);

/// Mark this process as the app.
pub fn as_app() {
    APP.store(true, Ordering::Relaxed);
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
    sync_some(None)
}

/// Load (or reload) plugin `id` only, leaving the others as they are: a terminal's `echoctl plugin call` runs no other
/// plugin's start-up code.
pub fn sync_only(id: &str) {
    sync_some(Some(id))
}

fn sync_some(only: Option<&str>) {
    let want: BTreeMap<String, PathBuf> = plugins::enabled()
        .into_iter()
        .filter(|p| only.is_none_or(|o| o == p.id))
        .filter_map(|p| {
            let dir = plugins::plugin_dir(&p);
            let m = plugins::read_manifest(&dir).ok()?;
            m.main.as_ref().map(|_| (p.id.clone(), dir))
        })
        .collect();
    let h = host();
    let have: Vec<String> = h.loaded.lock().map(|l| l.keys().cloned().collect()).unwrap_or_default();
    for id in have.iter().filter(|id| only.is_none() && !want.contains_key(*id)) {
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
        sync_only(plugin);
    }
    let (tx, rx) = channel();
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    h.tx.send(Msg::Call { plugin: plugin.into(), command: command.into(), args: args.to_string(), n, reply: tx }).map_err(|_| anyhow!("the plugins' host stopped"))?;
    match rx.recv_timeout(timeout) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => bail!("{e}"),
        Err(_) => {
            let _ = h.tx.send(Msg::Forget(plugin.into(), n));
            bail!("{plugin} didn't answer {command} within {} s", timeout.as_secs())
        }
    }
}

/// Tell the loaded plugins that asked for it (`uses.events.on`) about something that happened in Echo (`ctx.on`).
pub fn event(name: &str, payload: &Value) {
    if let Some(h) = HOST.get() {
        let _ = h.tx.send(Msg::Event(name.into(), payload.to_string()));
    }
}

/// The same, each plugin with its own part (`each[plugin]`'s fields set on `payload`).
pub fn event_each(name: &str, payload: &Value, each: &serde_json::Map<String, Value>) {
    if let Some(h) = HOST.get() {
        let _ = h.tx.send(Msg::EventEach(name.into(), payload.clone(), each.clone()));
    }
}

/// This process is the app and runs the plugins' host: an event sent here reaches the plugins the user sees.
pub fn hosts_here() -> bool {
    HOST.get().is_some() && APP.load(Ordering::Relaxed)
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
    /// Its work provider runs now (Echo's `__work` ask, until that slice ends): a `ctx.work.changed()` from it is not
    /// heard — the answer it is giving is the change (heard, every ask would ask again).
    providing: bool,
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
                hears().lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                crate::grants::drop_plugin(&id);
                if let Ok(mut l) = host().loaded.lock() {
                    l.remove(&id);
                }
            }
            Ok(Msg::Call { plugin, command, args, n, reply }) => {
                let Some(p) = plugins.get_mut(&plugin) else {
                    let why = status().get(&plugin).and_then(|r| r.clone().err()).unwrap_or_else(|| "it isn't installed and on".into());
                    let _ = reply.send(Err(format!("{plugin} isn't loaded: {why}")));
                    continue;
                };
                p.calls.insert(n, reply);
                set_current(&current, &plugin);
                current.lock().unwrap().providing = command == "__work";
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
                current.lock().unwrap().providing = false;
            }
            Ok(Msg::Forget(plugin, n)) => {
                if let Some(p) = plugins.get_mut(&plugin) {
                    p.calls.remove(&n);
                }
            }
            Ok(Msg::Event(name, payload)) => {
                let heard = hears().lock().unwrap_or_else(|e| e.into_inner()).clone();
                for (id, p) in plugins.iter().filter(|(id, _)| heard.get(*id).is_some_and(|h| h.contains(&name))) {
                    set_current(&current, id);
                    let ctx = p.ctx.clone();
                    slice(&mut || ctx.with(|c| drop(c.globals().get::<_, Function>("__event").and_then(|f| f.call::<_, ()>((name.clone(), payload.clone()))))));
                }
            }
            Ok(Msg::EventEach(name, base, each)) => {
                let heard = hears().lock().unwrap_or_else(|e| e.into_inner()).clone();
                for (id, p) in plugins.iter().filter(|(id, _)| heard.get(*id).is_some_and(|h| h.contains(&name))) {
                    let mut payload = base.clone();
                    for (k, v) in each.get(id).and_then(Value::as_object).into_iter().flatten() {
                        payload[k] = v.clone();
                    }
                    let payload = payload.to_string();
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
            // What another process changed (a thread deleted with `echoctl project delete`): the app's plugins hear
            // it — the app's alone, never a terminal's host.
            if APP.load(Ordering::Relaxed) {
                plugins::thread_values::take_queued(event_each);
            }
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
        #[cfg(test)]
        if let Ok(mut w) = tests::WAITING.lock() {
            *w = plugins.iter().map(|(id, p)| (id.clone(), p.calls.len())).collect();
        }
    }
}

fn set_current(current: &Mutex<Current>, id: &str) {
    if let Ok(mut c) = current.lock() {
        c.id = id.into();
    }
}

/// The events each loaded plugin hears (`plugins::heard_events`, set as it loads): what `ctx.on` checks and what
/// the host delivers — one grant, read once from the manifest that loaded.
fn hears() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static HEARS: std::sync::OnceLock<Mutex<HashMap<String, Vec<String>>>> = std::sync::OnceLock::new();
    HEARS.get_or_init(Default::default)
}

fn load(rt: &Runtime, id: &str, dir: &Path, current: &Arc<Mutex<Current>>, tx: &Sender<Msg>, slice: &mut dyn FnMut(&mut dyn FnMut())) -> Result<Context> {
    let m = plugins::read_manifest(dir)?;
    hears().lock().unwrap_or_else(|e| e.into_inner()).insert(id.into(), plugins::heard_events(&m.uses));
    let main = m.main.ok_or_else(|| anyhow!("no host half"))?;
    sweep_store(id);
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
    if op == "work.changed" && current.lock().is_ok_and(|c| c.providing) {
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

/// A value in curl's config: quoted, its backslashes, quotes and line breaks escaped.
fn curl_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

/// The first lines of curl's config for `ctx.http.fetch`. Only http(s), never a redirect elsewhere: what the plugin may
/// reach is checked on this URL alone. A request sent to a fake service never goes through a proxy (`http_proxy`,
/// `all_proxy`): it stays on this computer, its headers with it.
fn curl_head(url: &str, method: &str, to_fake: bool) -> String {
    let mut cfg = format!("url = {}\nrequest = \"{method}\"\nproto = \"=http,https\"\ngloboff\nmax-time = 60\nmax-filesize = 10485760\nwrite-out = \"\\n%{{http_code}}\"\n", curl_quote(url));
    if to_fake {
        cfg.push_str("noproxy = \"*\"\n");
    }
    cfg
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

/// A plugin command a read-only routine ran (`echoctl plugin call`, its environment inherited): it reads, it changes
/// nothing outside the plugin's store — no folder writes, no Claude run, no message, no request but a GET.
fn read_only_here(what: &str) -> Result<()> {
    if std::env::var(crate::guard::RUN_MODE).is_ok_and(|m| m == "routine") {
        bail!("{what} isn't allowed in a read-only routine");
    }
    Ok(())
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

/// Not one of Echo's files in the plugin's store (written or removed only by Echo, under their locks): its settings,
/// its work rows, its values per thread.
fn not_echos(rel: &str) -> Result<()> {
    let first = Path::new(rel.trim_start_matches("./")).components().next().map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    match first.trim_end_matches(['.', ' ']).split(':').next() {
        Some("settings.json") => bail!("the plugin's settings.json isn't its to write: use ctx.settings.set"),
        Some(work::FILE) => bail!("the plugin's {} isn't its to write: Echo keeps what ctx.work.provide answers", work::FILE),
        Some(plugins::thread_values::FILE) => bail!("the plugin's {} isn't its to write: use ctx.threads.set", plugins::thread_values::FILE),
        _ => Ok(()),
    }
}

/// A store path the plugin may write: not its `settings.json`, which holds what the user set (the addresses it may
/// reach among them) and is changed only through `plugins::set_setting`.
fn writable_store_path(id: &str, rel: &str) -> Result<PathBuf> {
    store_path(id, rel)?;
    not_echos(rel)?;
    // Kept inside the store once links are followed, and no Windows device name.
    let base = plugins::data_dir(id);
    std::fs::create_dir_all(&base)?;
    crate::grants::inside(&dunce::canonicalize(&base)?, rel)
}

/// When a host half starts: what lay in its store's `cache/` over 7 days, and downloads left half-written (`.part`,
/// over an hour: a fetch takes at most 60 s), go. The rest of the store is the plugin's to clean.
fn sweep_store(id: &str) {
    fn walk(dir: &Path, f: &mut dyn FnMut(&Path, std::time::Duration)) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                walk(&e.path(), f);
            } else if t.is_file() {
                let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).unwrap_or_default();
                f(&e.path(), age);
            }
        }
    }
    const DAY: u64 = 86_400;
    let base = plugins::data_dir(id);
    let cache = base.join("cache");
    walk(&base, &mut |p, age| {
        // Only Echo's own downloads: `<name>.<pid>-<n>.part`.
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ours = name.strip_suffix(".part").and_then(|r| r.rsplit_once('.')).and_then(|(_, t)| t.split_once('-')).is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && (a.to_owned() + b).bytes().all(|c| c.is_ascii_digit()));
        let stale_part = ours && age.as_secs() > 3600;
        if stale_part || (p.starts_with(&cache) && age.as_secs() > 7 * DAY) {
            let _ = std::fs::remove_file(p);
        }
    });
}

fn service(id: &str, op: &str, arg: &Value) -> Result<Value> {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    Ok(match op {
        "id" => json!(id),
        "terminal" => json!(!TIMERS.load(Ordering::Relaxed)),
        "log" => {
            eprintln!("plugin {id}: [{}] {}", s(&arg[0]), s(&arg[1]));
            Value::Null
        }
        "events.on" => {
            let name = s(arg);
            if !hears().lock().unwrap_or_else(|e| e.into_inner()).get(id).is_some_and(|h| h.contains(&name)) {
                bail!("{id} didn't ask to hear “{name}” when it was installed (uses.events.on)");
            }
            json!(true)
        }
        "store.read" => match std::fs::read_to_string(store_path(id, &s(arg))?) {
            Ok(t) => json!(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
            Err(e) => return Err(e.into()),
        },
        "store.write" => {
            let p = writable_store_path(id, &s(&arg[0]))?;
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
            // Not the store itself, nor Echo's files in it (its work rows, settings, thread values: their locks aren't
            // taken here).
            if p == plugins::data_dir(id) {
                bail!("ctx.store.remove needs a path inside the plugin's folder, not the folder itself");
            }
            not_echos(&s(arg))?;
            if p.is_dir() {
                std::fs::remove_dir_all(&p)?;
            } else if p.exists() {
                std::fs::remove_file(&p)?;
            }
            Value::Null
        }
        "store.size" => {
            let base = plugins::data_dir(id);
            let dir = if s(arg).is_empty() { base.clone() } else { store_path(id, &s(arg))? };
            let (mut bytes, mut files) = (0u64, 0u64);
            fn walk(d: &Path, f: &mut dyn FnMut(&Path, u64)) {
                for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                    let Ok(t) = e.file_type() else { continue };
                    if t.is_dir() {
                        walk(&e.path(), f);
                    } else if t.is_file() {
                        f(&e.path(), e.metadata().map(|m| m.len()).unwrap_or(0));
                    }
                }
            }
            // The user's settings, written by Echo, aren't the plugin's.
            walk(&dir, &mut |p, n| {
                if p != base.join("settings.json") {
                    bytes += n;
                    files += 1;
                }
            });
            json!({"bytes": bytes, "files": files})
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
        "widgets.set" => {
            widget_store(id, &s(&arg[0]), Some(&arg[1]), None, None)?;
            Value::Null
        }
        "widgets.clear" => {
            widget_store(id, &s(arg), None, None, None)?;
            Value::Null
        }
        "work.provide" => {
            uses(id, "work")?;
            Value::Null
        }
        "work.changed" => {
            uses(id, "work")?;
            work::changed(id);
            Value::Null
        }
        "threads.get" => plugins::thread_values::get(id, &s(&arg[0]), &s(&arg[1]))?,
        "threads.set" => {
            read_only_here("changing files or settings")?;
            plugins::thread_values::set(id, &s(&arg[0]), &s(&arg[1]), &arg[2])?;
            Value::Null
        }
        "threads.list" => plugins::thread_values::list(id, &s(arg))?,
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
            read_only_here("changing files or settings")?;
            let m = plugins::read_manifest(&package_dir(id)?)?;
            if plugins::address_settings(&m.uses).contains(&s(&arg[0])) {
                bail!("only you set the addresses {id} may reach (its setting “{}”)", s(&arg[0]));
            }
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
            let (kind, opt) = match arg {
                Value::Array(a) => (a.first().and_then(|k| k.as_str()).unwrap_or("").to_string(), a.get(1).cloned().unwrap_or(Value::Null)),
                _ => (s(arg), Value::Null),
            };
            // `source` is read under `sources`'s grant too, and the other way round: both are the same data.
            let asked = |k: &str| uses(id, "echo").is_ok_and(|u| u["read"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(k))));
            let granted = asked(&kind) || (matches!(kind.as_str(), "source" | "sources") && (asked("source") || asked("sources")));
            if !granted {
                bail!("{id} didn't ask to read your {kind} when it was installed");
            }
            match kind.as_str() {
                "source" => crate::share::source(opt.as_str().unwrap_or(""))?,
                "sources" => crate::share::sources(&opt)?,
                "people" => crate::share::people()?,
                "dictionary" => crate::share::dictionary()?,
                "clients" => json!(crate::projects::clients().unwrap_or_default().into_iter().map(|c| json!({"id": c.id, "name": c.name, "aliases": c.aliases, "domains": c.domains})).collect::<Vec<_>>()),
                "threads" => json!(crate::projects::list().into_iter().filter(|p| !p.stage.closed()).map(|p| json!({"id": p.id, "name": p.name, "client": p.client})).collect::<Vec<_>>()),
                o => bail!("Echo doesn't share “{o}” (clients, threads, people, dictionary, sources, source)"),
            }
        }
        "echo.dictionary.add" => {
            read_only_here("changing files or settings")?;
            if !uses(id, "echo")?["write"].as_array().is_some_and(|a| a.iter().any(|k| k == "dictionary")) {
                bail!("{id} didn't ask to add to your dictionary when it was installed");
            }
            crate::share::dictionary_add(id, arg)?
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
        "fs.write" => {
            read_only_here("changing files or settings")?;
            uses_flag(id, "folders", "write")?;
            let f = crate::grants::writable(id, &s(&arg[0]), &s(&arg[1]))?;
            if f.is_dir() {
                bail!("“{}” is a folder", s(&arg[1]));
            }
            std::fs::create_dir_all(f.parent().unwrap_or(&f))?;
            paths::write_atomic(&f, s(&arg[2]).as_bytes())?;
            Value::Null
        }
        "fs.move" => {
            read_only_here("changing files or settings")?;
            uses_flag(id, "folders", "write")?;
            let from = crate::grants::writable(id, &s(&arg[0]), &s(&arg[1]))?;
            let to = crate::grants::writable(id, &s(&arg[0]), &s(&arg[2]))?;
            if !from.exists() {
                bail!("“{}” isn't there", s(&arg[1]));
            }
            // There already — unless it is the same file under another case (a rename on a case-insensitive disk).
            let same = dunce::canonicalize(&to).ok().is_some_and(|t| dunce::canonicalize(&from).ok() == Some(t));
            if to.exists() && !same {
                bail!("“{}” is there already", s(&arg[2]));
            }
            std::fs::create_dir_all(to.parent().unwrap_or(&to))?;
            std::fs::rename(&from, &to)?;
            Value::Null
        }
        "fs.remove" => {
            read_only_here("changing files or settings")?;
            uses_flag(id, "folders", "write")?;
            let f = crate::grants::writable(id, &s(&arg[0]), &s(&arg[1]))?;
            if f.is_dir() {
                std::fs::remove_dir(&f).map_err(|_| anyhow!("“{}” is a folder with files in it", s(&arg[1])))?;
            } else {
                std::fs::remove_file(&f)?;
            }
            Value::Null
        }
        "files.read" => {
            let entry = s(arg);
            if !uses(id, "files")?["read"].as_array().is_some_and(|a| a.iter().any(|f| f.as_str() == Some(entry.as_str()))) {
                bail!("{id} didn't name “{entry}” in uses.files.read when it was installed");
            }
            // The file as the sheet named it: no link anywhere below its base (`plugins::app_file_real`).
            match plugins::app_file_real(&entry, |k| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from))? {
                Some(f) => {
                    use std::io::Read;
                    let mut buf = vec![];
                    std::fs::File::open(&f)?.take(1 << 20).read_to_end(&mut buf)?;
                    json!(String::from_utf8_lossy(&buf))
                }
                None => Value::Null,
            }
        }
        "system.which" => {
            let name = s(arg);
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                bail!("“{name}” isn't a program name");
            }
            let p = crate::app::find_program(&name);
            if Path::new(&p).is_file() { json!(p) } else { Value::Null }
        }
        "signin.sources" => {
            plugins::may_use_signin(&plugins::read_manifest(&package_dir(id)?)?.uses, &s(arg))?;
            plugins::signin::sources()
        }
        o => bail!("Echo has no service “{o}”"),
    })
}

fn service_async(id: &str, op: &str, arg: &Value) -> Result<Value> {
    match op {
        // A Claude Code run on one of the user's AI profiles: Echo's guarded run (`claudejob.rs`).
        "claude.run" => {
            read_only_here("a Claude run")?;
            // Only what its install sheet said: running Claude Code at all, and with permission checks skipped.
            plugins::may_run_claude(&plugins::read_manifest(&package_dir(id)?)?.uses, arg["tools"].as_str().unwrap_or(""))?;
            let spec = crate::claudejob::Spec::from_value(arg, &plugins::data_dir(id))?;
            let (answer, model, usage) = crate::claudejob::run(&spec)?;
            Ok(json!({"answer": answer, "model": model, "usage": usage}))
        }
        // The user's sign-in to a host its sheet named (`plugins::signin`), to the host half only.
        "signin.get" => {
            let host = arg[0].as_str().unwrap_or("");
            plugins::may_use_signin(&plugins::read_manifest(&package_dir(id)?)?.uses, host)?;
            read_only_here("reading your sign-in")?;
            Ok(plugins::signin::get(host, arg[1].as_bool().unwrap_or(false)).unwrap_or(Value::Null))
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
            read_only_here("sending")?;
            let kind = arg[0].as_str().unwrap_or("");
            // Only the ways its install sheet named, or its own.
            plugins::may_send(&plugins::read_manifest(&package_dir(id)?)?.uses, kind)?;
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

// ── the plugins' Today widgets ──

fn widgets_file(id: &str) -> PathBuf {
    plugins::data_dir(id).join("widgets.json")
}

/// A widget's last data (`None`: cleared), or why its provider couldn't answer (the data before kept) — one the
/// plugin declares, at most 64 KB; `size`: the size its provider was asked for.
fn widget_store(id: &str, widget: &str, data: Option<&Value>, error: Option<&str>, size: Option<&str>) -> Result<()> {
    let declared = plugins::enabled_manifests().into_iter().find(|(p, _)| p == id).is_some_and(|(_, m)| m.widgets.iter().any(|w| w.id == widget));
    if !declared {
        bail!("{widget:?} isn't a widget this plugin declares (`widgets` in its manifest)");
    }
    if data.is_some_and(|d| d.to_string().len() > 64 * 1024) {
        bail!("a widget's data is 64 KB at most");
    }
    let f = widgets_file(id);
    let _lock = WIDGETS.lock().unwrap_or_else(|e| e.into_inner());
    let mut all: serde_json::Map<String, Value> = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    // `at`: when its data was given (Today's "from HH:MM"); `asked`: when Echo last asked its provider (or found it
    // had none), for the next ask; `error`: why the last ask failed, its data kept.
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut last = all.get(widget).cloned().unwrap_or_else(|| json!({"data": null}));
    match (data, error) {
        (_, Some("")) => {
            last["asked"] = json!(now);
        }
        (_, Some(e)) => {
            last["error"] = json!(e);
            last["asked"] = json!(now);
        }
        (d, None) => {
            // A `set` keeps the size its provider was last asked for (else Today would take it for a resize).
            let kept = last.get("size").cloned();
            last = json!({"data": d.cloned().unwrap_or(Value::Null), "at": now, "asked": now});
            if let Some(k) = kept {
                last["size"] = k;
            }
        }
    }
    if let Some(size) = size {
        last["size"] = json!(size);
    }
    all.insert(widget.into(), last);
    std::fs::create_dir_all(plugins::data_dir(id))?;
    paths::write_atomic(&f, serde_json::to_string_pretty(&all)?.as_bytes())?;
    Ok(())
}

static WIDGETS: Mutex<()> = Mutex::new(());
/// The widgets being asked now (`plugin:widget`), so a busy Today asks each once at a time.
static ASKING: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Widgets asked for again while being asked: one more ask once this one is in.
static AGAIN: Mutex<std::collections::BTreeSet<String>> = Mutex::new(std::collections::BTreeSet::new());

/// A widget's last data, when it was given, when Echo last asked, and its provider's last error:
/// `{data, at, asked, error?}`.
pub fn widget_last(plugin: &str, widget: &str) -> Option<Value> {
    let all: serde_json::Map<String, Value> = std::fs::read(widgets_file(plugin)).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
    all.get(widget).cloned()
}

/// Ask a widget's provider for its data in the background (5 s, as a command), keeping the answer — or, when it has
/// no provider (it only `set`s) or fails, its last data with why.
pub fn widget_ask(plugin: &str, widget: &str) {
    let key = format!("{plugin}:{widget}");
    {
        let mut asking = ASKING.lock().unwrap_or_else(|e| e.into_inner());
        if asking.contains(&key) {
            // Asked while an ask is on its way (a setting just changed): asked once more after it.
            AGAIN.lock().unwrap_or_else(|e| e.into_inner()).insert(key);
            return;
        }
        asking.push(key.clone());
    }
    let (plugin, widget) = (plugin.to_string(), widget.to_string());
    std::thread::spawn(move || {
        loop {
            widget_ask_now(&plugin, &widget);
            if !AGAIN.lock().unwrap_or_else(|e| e.into_inner()).remove(&key) {
                break;
            }
        }
        ASKING.lock().unwrap_or_else(|e| e.into_inner()).retain(|k| k != &key);
    });
}

/// Ask a widget's provider and wait (5 s at most) — `echoctl widget show`, where a plugin's author checks its answer.
pub fn widget_ask_now(plugin: &str, widget: &str) {
    // Its settings as Today has them (defaults, then the user's), given to the provider.
    // and the size it has there (S, M, L), so it can give as much as fits.
    let id = format!("plugin:{plugin}:{widget}");
    let (params, size) = (crate::today_layout::params_of(&id), crate::today_layout::size_on_today(&id));
    let size = Some(size.as_str());
    match call_within(plugin, &format!("__widget:{widget}"), &json!({ "params": params, "size": size }), Duration::from_secs(5)) {
        // Kept — or, refused (over 64 KB), why, its data before kept.
        Ok(v) => {
            if let Err(e) = widget_store(plugin, widget, Some(&v), None, size) {
                let _ = widget_store(plugin, widget, None, Some(&format!("{e:#}")), size);
            }
        }
        // Only `set`: nothing to ask; its data stays as the plugin last gave it, marked asked (not asked again before
        // its refresh) — under the lock, so a `set` meanwhile isn't overwritten.
        Err(e) if e.to_string().contains(&format!("no command “__widget:{widget}”")) => drop(widget_store(plugin, widget, None, Some(""), size)),
        Err(e) => drop(widget_store(plugin, widget, None, Some(&format!("{e:#}")), size)),
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

    /// The calls each loaded plugin still owes a reply (the host thread's, after each message).
    pub(super) static WAITING: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());

    fn got_size(plugin: &str, widget: &str) -> String {
        widget_last(plugin, widget).and_then(|l| l["data"]["items"][0]["sub"].as_str().map(str::to_owned)).unwrap_or_default()
    }

    /// The size its provider last answered for, once it is `want` (or what it was after 5 s).
    fn wait_size(plugin: &str, widget: &str, want: &str) -> String {
        for _ in 0..100 {
            if got_size(plugin, widget) == want {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        got_size(plugin, widget)
    }

    #[test]
    fn a_plugins_widgets_are_in_the_catalog_and_give_their_data_by_provide_and_set() {
        let (_env, root) = paths::testenv::folders("jshost-widgets");
        let mk = root.join("market");
        let d = mk.join("plugins/ci");
        std::fs::create_dir_all(&d).unwrap();
        let widgets = json!([
            {"id": "builds", "label": "Builds", "about": "the last builds", "kind": "list", "sizes": ["M", "L"]},
            {"id": "health", "label": "CI health", "kind": "status", "params": [{"key": "branch", "label": "Branch", "type": "choice", "options": ["main", "dev"], "default": "main"}]},
        ]);
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "ci", "name": "CI", "version": "0.1.0", "main": "main.js", "widgets": widgets}).to_string()).unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              ctx.widgets.provide('builds', ({ size }) => ({ items: [{ title: 'main #41 passed', sub: `at ${size}` }] }))
              ctx.widgets.provide('health', ({ params }) => ({ state: 'ok', title: `CI on ${params.branch}` }))
              ctx.command('green', () => { ctx.widgets.set('health', { state: 'ok', title: 'All green' }); return 1 })
              ctx.command('stranger', () => ctx.widgets.set('nope', { state: 'ok' }))
              ctx.command('off', () => ctx.widgets.clear('health'))
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "ci", "source": "./plugins/ci"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review("ci", Some(&name)).unwrap().uses;
        plugins::install("ci", Some(&name), &agreed).unwrap();
        sync();
        // In the catalog under the plugin's name, at the sizes it declared.
        let w = crate::widgets::find("plugin:ci:builds").unwrap();
        assert_eq!((w.part.as_ref(), w.label.as_ref(), w.kind.as_ref(), w.sizes.to_vec()), ("CI", "Builds", "list", vec!["M", "L"]));
        // Pushed: set, then clear; a widget it doesn't declare is refused.
        let green = (0..50).find_map(|_| call("ci", "green", &json!({})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(green, Some(json!(1)));
        assert_eq!(widget_last("ci", "health").unwrap()["data"]["title"], "All green");
        assert!(call("ci", "stranger", &json!({})).unwrap_err().to_string().contains("isn't a widget this plugin declares"));
        // A `set` stays: Today doesn't take it for a resize and ask the provider over it.
        crate::today_layout::add("plugin:ci:health", Some("M"), None).unwrap();
        widget_ask_now("ci", "health");
        call("ci", "green", &json!({})).unwrap();
        crate::widgets::data("plugin:ci:health", &crate::widgets::Inputs { tasks: &json!({}) }).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(widget_last("ci", "health").unwrap()["data"]["title"], "All green", "not asked over");
        crate::today_layout::remove("plugin:ci:health").unwrap();
        call("ci", "off", &json!({})).unwrap();
        assert!(widget_last("ci", "health").unwrap()["data"].is_null());
        // Pulled: Echo asks its provider (in the background) and keeps the answer.
        assert!(widget_last("ci", "builds").is_none());
        let first = crate::widgets::data("plugin:ci:builds", &crate::widgets::Inputs { tasks: &json!({}) }).unwrap();
        assert_eq!(first["empty"], true, "nothing yet: drawn once the provider answers");
        assert_eq!(first["pending"], true, "never answered: Today holds its place");
        let got = (0..100).find_map(|_| widget_last("ci", "builds").or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(got.unwrap()["data"]["items"][0]["title"], "main #41 passed");
        let v = crate::widgets::data("plugin:ci:builds", &crate::widgets::Inputs { tasks: &json!({}) }).unwrap();
        assert_eq!((v["kind"].as_str(), v["label"].as_str(), v["empty"].as_bool()), (Some("list"), Some("Builds"), Some(false)));
        assert_eq!(v["pending"], false);
        // Answered with nothing (cleared): no place, not a wait.
        let off = crate::widgets::data("plugin:ci:health", &crate::widgets::Inputs { tasks: &json!({}) }).unwrap();
        assert_eq!((off["empty"].as_bool(), off["pending"].as_bool()), (Some(true), Some(false)));
        assert_eq!(got_size("ci", "builds"), "at M", "off Today: the first size it takes");
        // Today takes it like Echo's own; its provider is told the size it has there, and asked again when resized.
        crate::today_layout::add("plugin:ci:builds", Some("L"), None).unwrap();
        assert!(crate::today_layout::load().iter().any(|i| i.widget == "plugin:ci:builds" && i.size == "L"));
        crate::widgets::data("plugin:ci:builds", &crate::widgets::Inputs { tasks: &json!({}) }).unwrap();
        assert_eq!(wait_size("ci", "builds", "at L"), "at L", "added at L: asked again although not stale");
        crate::today_layout::resize("plugin:ci:builds", "M").unwrap();
        crate::widgets::data("plugin:ci:builds", &crate::widgets::Inputs { tasks: &json!({}) }).unwrap();
        assert_eq!(wait_size("ci", "builds", "at M"), "at M", "resized to M");
        // Its settings: declared in the manifest, set on Today, given to its provider.
        crate::today_layout::add("plugin:ci:health", Some("M"), None).unwrap();
        widget_ask_now("ci", "health");
        assert_eq!(widget_last("ci", "health").unwrap()["data"]["title"], "CI on main", "the default");
        crate::today_layout::set_param("plugin:ci:health", "branch", "dev").unwrap();
        widget_ask_now("ci", "health");
        assert_eq!(widget_last("ci", "health").unwrap()["data"]["title"], "CI on dev");
        assert!(crate::today_layout::set_param("plugin:ci:health", "branch", "prod").is_err());
        plugins::remove("ci", false).unwrap();
        sync();
    }

    #[test]
    fn a_manifests_widgets_are_checked() {
        let (_env, root) = paths::testenv::folders("jshost-widget-manifest");
        let d = root.join("p");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("main.js"), "export function apply(ctx) {}").unwrap();
        let with = |w: Value, main: bool| {
            let mut m = json!({"id": "p", "name": "P", "version": "0.1.0", "widgets": [w]});
            if main {
                m["main"] = json!("main.js");
            }
            std::fs::write(d.join("echo-plugin.json"), m.to_string()).unwrap();
            plugins::read_manifest(&d).map(|_| ()).map_err(|e| e.to_string())
        };
        assert!(with(json!({"id": "a", "label": "A", "kind": "stat"}), true).is_ok());
        assert!(with(json!({"id": "a", "label": "A", "kind": "map"}), true).unwrap_err().contains("kind Echo doesn't draw"));
        assert!(with(json!({"id": "A b", "label": "A", "kind": "stat"}), true).unwrap_err().contains("plain, unique id"));
        assert!(with(json!({"id": "a", "label": "A", "kind": "stat", "sizes": ["XL"]}), true).unwrap_err().contains("sizes of S, M and L"));
        assert!(with(json!({"id": "a", "label": "A", "kind": "stat", "refresh": 5}), true).unwrap_err().contains("60 s"));
        assert!(with(json!({"id": "a", "label": "A", "kind": "stat", "heights": [1, 2]}), true).is_ok());
        for bad in [json!([3]), json!([]), json!([2, 2]), json!([2])] {
            assert!(with(json!({"id": "a", "label": "A", "kind": "stat", "heights": bad}), true).unwrap_err().contains("heights of 1 and 2 rows"));
        }
        assert!(with(json!({"id": "a", "label": "A", "kind": "stat"}), false).unwrap_err().contains("host half"));
        assert!(with(json!({"id": "a", "label": "A", "kind": "page"}), true).unwrap_err().contains("needs a window half (client)"));
        let param = |p: Value| with(json!({"id": "a", "label": "A", "kind": "stat", "params": [p]}), true);
        assert!(param(json!({"key": "team", "label": "Team", "type": "thread"})).is_ok());
        assert!(param(json!({"key": "team", "label": "Team", "type": "colour"})).unwrap_err().contains("type Echo doesn't know"));
        assert!(param(json!({"key": "k", "label": "K", "type": "choice"})).unwrap_err().contains("choice of 1–12 options"));
        assert!(param(json!({"key": "k", "label": "K", "type": "choice", "options": ["a"], "default": "b"})).unwrap_err().contains("default isn't one it takes"));
    }

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
              ctx.command('size', () => { ctx.store.write('cache/clip.mp3', '12345'); return [ctx.store.size(), ctx.cache.size(), ctx.store.size('notes')] })
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
        // What it keeps, all of it or its cache: its own files only — the user's settings, written by Echo, aren't counted.
        std::fs::write(plugins::data_dir("hello").join("settings.json"), "{\"k\": \"a long value\"}").unwrap();
        assert_eq!(call("hello", "size", &json!({})).unwrap(), json!([{"bytes": 6, "files": 2}, {"bytes": 5, "files": 1}, {"bytes": 1, "files": 1}]));
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
    fn a_development_build_sends_a_named_host_to_a_fake_service_and_nothing_else() {
        let hosts = vec!["api.github.com".to_string(), "uploads.github.com".to_string()];
        let to = |map: &str, url: &str| test_host_in(map, url, &hosts).map_err(|e| e.to_string());
        let map = " api.github.com = http://127.0.0.1:47990/ , other.example=HTTP://LocalHost:47991,";
        assert_eq!(to(map, "https://api.github.com/user/repos?per_page=5"), Ok(Some("http://127.0.0.1:47990/user/repos?per_page=5".into())));
        assert_eq!(to(map, "https://api.github.com"), Ok(Some("http://127.0.0.1:47990".into())));
        assert_eq!(to("api.github.com=http://localhost:8080/api/", "https://api.github.com/x"), Ok(Some("http://localhost:8080/api/x".into())));
        assert_eq!(to("API.GitHub.com=Http://LOCALHOST:8080", "https://api.github.com/x"), Ok(Some("http://localhost:8080/x".into())), "written in any case");
        // Not a named https host: an address the user listed, left as it is (its grant was checked on it).
        assert_eq!(to(map, "https://other.example/x"), Ok(None));
        assert_eq!(to(map, "http://api.github.com/x"), Ok(None));
        assert_eq!(to(map, "http://127.0.0.1:8080/x"), Ok(None));
        // A named host the variable doesn't map never goes out: not to the real host.
        let unmapped = to(map, "https://uploads.github.com/x").unwrap_err();
        assert!(unmapped.contains("isn't mapped in ECHOCTL_TEST_HOSTS: a test never reaches the real uploads.github.com"), "{unmapped}");
        assert!(to("", "https://api.github.com/x").unwrap_err().contains("isn't mapped"), "set but empty: nothing mapped");
        // A base that isn't a local address fails the request, whichever host it names.
        for base in [
            "http://[::1]:47990",
            "http://0.0.0.0:47990",
            "https://127.0.0.1:47990",
            "http://example.com:1",
            "http://127.0.0.1:1@evil.example",
            "http://user@127.0.0.1:1",
            "http://127.0.0.1.evil.example:1",
            "http://localhost:x",
            "http://127.0.0.1:",
            "http://127.0.0.1",
            "http://localhost:0",
            "http://localhost:65536",
            "http://127.0.0.1:1/a b",
            "http://127.0.0.1:1/?x",
            "127.0.0.1:1",
        ] {
            for map in [format!("api.github.com={base}"), format!("api.github.com=http://127.0.0.1:1,other.example={base}")] {
                let e = to(&map, "https://api.github.com/x").unwrap_err();
                assert!(e.starts_with("ECHOCTL_TEST_HOSTS: ") && e.contains("isn't a local address"), "{map}: {e}");
            }
        }
        for map in ["api.github.com", "api.github.com:443=http://127.0.0.1:1", "=http://127.0.0.1:1", "api.github.com=http://127.0.0.1:1,API.github.com=http://127.0.0.1:2"] {
            assert!(to(map, "https://api.github.com/x").unwrap_err().starts_with("ECHOCTL_TEST_HOSTS: "), "{map}");
        }
    }

    #[test]
    fn a_request_to_a_fake_service_never_goes_through_a_proxy() {
        let port = serve_clips();
        let url = format!("http://127.0.0.1:{port}/text");
        let run = |to_fake: bool| {
            let mut child = std::process::Command::new("curl")
                .args(["-q", "-sS", "-K", "-"])
                // Only on this curl: a proxy that isn't there, which every request would go through.
                .env("http_proxy", "http://127.0.0.1:1")
                .env("all_proxy", "http://127.0.0.1:1")
                .env_remove("no_proxy")
                .env_remove("NO_PROXY")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            std::io::Write::write_all(&mut child.stdin.take().unwrap(), curl_head(&url, "GET", to_fake).as_bytes()).unwrap();
            let out = child.wait_with_output().unwrap();
            (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
        };
        assert_eq!(run(true), (true, "hello\n200".to_string()));
        assert!(!run(false).0, "without `noproxy` curl takes the proxy, even for 127.0.0.1");
    }

    /// Sets an environment variable until dropped, even when the test panics.
    #[cfg(debug_assertions)]
    struct EnvVar(&'static str);

    #[cfg(debug_assertions)]
    impl EnvVar {
        fn set(name: &'static str, value: &str) -> Self {
            std::env::set_var(name, value);
            EnvVar(name)
        }
    }

    #[cfg(debug_assertions)]
    impl Drop for EnvVar {
        fn drop(&mut self) {
            std::env::remove_var(self.0);
        }
    }

    // A release build reads no ECHOCTL_TEST_HOSTS: there this would reach for the real hosts.
    #[cfg(debug_assertions)]
    #[test]
    fn a_plugins_named_host_reaches_the_fake_service_set_for_tests() {
        let port = serve_clips();
        let (_env, root) = paths::testenv::folders("jshost-test-hosts");
        let mk = root.join("market");
        let d = mk.join("plugins/hub");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("echo-plugin.json"),
            json!({"id": "hub", "name": "Hub", "version": "0.1.0", "main": "main.js",
                   "uses": {"network": {"hosts": ["api.acme.example", "uploads.acme.example"], "hosts_from": ["tts"]}},
                   "settings": [{"key": "tts", "label": "TTS server", "type": "string"}]})
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            d.join("main.js"),
            "export function apply(ctx) { ctx.command('ping', () => 'pong'); ctx.command('get', async (a) => await ctx.http.fetch(a.url)); ctx.command('save', async (a) => await ctx.http.fetch(a.url, { save: a.to })) }",
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "hub", "source": "./plugins/hub"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        plugins::install("hub", Some(&name), &plugins::review("hub", Some(&name)).unwrap().uses).unwrap();
        plugins::set_setting("hub", "tts", &json!(format!("http://127.0.0.1:{port}"))).unwrap();
        sync();
        // Loaded first, with no request: the retries never reach for a host.
        let up = (0..50).find_map(|_| call("hub", "ping", &json!({})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(up, Some(json!("pong")));
        let get = |url: &str| call("hub", "get", &json!({"url": url}));
        let refused = |r: Result<Value>| r.unwrap_err().to_string();
        let _map = EnvVar::set("ECHOCTL_TEST_HOSTS", &format!(" api.acme.example = http://127.0.0.1:{port} , other.example=http://127.0.0.1:{port}"));
        assert_eq!(get("https://api.acme.example/text").unwrap(), json!({"status": 200, "body": "hello"}));
        // A download comes through the fake service too.
        let saved = call("hub", "save", &json!({"url": "https://api.acme.example/clip", "to": "audio/a.mp3"})).unwrap();
        assert_eq!(saved, json!({"status": 200, "body": "", "saved": "audio/a.mp3", "bytes": 6}));
        assert_eq!(std::fs::read(plugins::data_dir("hub").join("audio/a.mp3")).unwrap(), vec![0xff, 0xfb, 0x90, 0x00, 0x80, 0xfe]);
        // The grant is checked first, on the real host.
        assert!(refused(get("https://other.example/text")).contains("may reach only the hosts its manifest names"));
        // A named host the variable doesn't map is refused, not sent to the real one.
        assert!(refused(get("https://uploads.acme.example/text")).contains("isn't mapped in ECHOCTL_TEST_HOSTS"));
        // The user's own address (`hosts_from`) is reached as it is.
        assert_eq!(get(&format!("http://127.0.0.1:{port}/text")).unwrap(), json!({"status": 200, "body": "hello"}));
        // A mapping that isn't a local address fails every request.
        let _map = EnvVar::set("ECHOCTL_TEST_HOSTS", &format!("api.acme.example=http://127.0.0.1:{port},uploads.acme.example=http://0.0.0.0:{port}"));
        assert!(refused(get("https://api.acme.example/text")).contains("isn't a local address"));
        drop(_map);
        plugins::remove("hub", false).unwrap();
    }

    /// The gates on every system; the answers (and `{fresh: true}`) where made-up programs are shell scripts.
    #[test]
    fn a_plugin_is_handed_a_sign_in_only_for_the_hosts_its_sheet_named_and_never_in_a_read_only_routine() {
        let (_env, root) = paths::testenv::folders("jshost-signin");
        // Made-up `gh` and `git` (Echo's tests never look for the computer's own: `signin::FAKE_PROGRAMS`), each
        // logging that it was asked; on Windows none (no program is asked there).
        let bin = root.join("fakebin");
        std::fs::create_dir_all(&bin).unwrap();
        #[cfg(unix)]
        let fake = |name: &str, body: &str| {
            use std::os::unix::fs::PermissionsExt;
            let p = bin.join(name);
            std::fs::write(&p, format!("#!/bin/sh\necho asked >> '{}.log'\n{body}\n", p.display())).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };
        #[cfg(unix)]
        let programs = vec![("gh", fake("gh", "echo gho_madeup777")), ("git", fake("git", "exit 1"))];
        #[cfg(not(unix))]
        let programs = Vec::new();
        *plugins::signin::FAKE_PROGRAMS.lock().unwrap() = programs;
        let asked = |name: &str| bin.join(format!("{name}.log")).exists();
        let mk = root.join("market");
        let d = mk.join("plugins/gh-ish");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "gh-ish", "name": "Acme hub", "version": "0.1.0", "main": "main.js", "uses": {"signin": {"hosts": ["signin-jshost.example"]}, "network": {"hosts": ["api.github.com"]}}}).to_string()).unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              ctx.command('get', async (a) => await ctx.signin.get(a.host))
              ctx.command('fresh', async (a) => await ctx.signin.get(a.host, { fresh: true }))
              ctx.command('sources', (a) => ctx.signin.sources(a.host))
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "gh-ish", "source": "./plugins/gh-ish"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let review = plugins::review("gh-ish", Some(&name)).unwrap();
        plugins::install("gh-ish", Some(&name), &review.uses).unwrap();
        sync();
        let sources = (0..50).find_map(|_| call("gh-ish", "sources", &json!({"host": "signin-jshost.example"})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        // Which programs are here, no sign-in read.
        assert_eq!(sources.unwrap(), json!({"gh": cfg!(unix), "git": cfg!(unix)}));
        assert!(!asked("gh") && !asked("git"));
        // Any other host is refused before a program is asked. (A refusal is matched, never an answer printed.)
        for host in ["gitlab.com", "api.github.com"] {
            for command in ["get", "fresh", "sources"] {
                match call("gh-ish", command, &json!({"host": host})) {
                    Err(e) => assert!(e.to_string().contains("didn't ask to use your sign-in"), "{host} {command}: {e}"),
                    Ok(_) => panic!("{command} answered for {host}"),
                }
            }
        }
        assert!(!asked("gh") && !asked("git"));
        // A read-only routine never reads one (refused before any program is asked).
        struct RunMode;
        impl Drop for RunMode {
            fn drop(&mut self) {
                std::env::remove_var(crate::guard::RUN_MODE);
            }
        }
        {
            let _routine = RunMode;
            std::env::set_var(crate::guard::RUN_MODE, "routine");
            for command in ["get", "fresh"] {
                match call("gh-ish", command, &json!({"host": "signin-jshost.example"})) {
                    Err(e) => assert!(e.to_string().contains("isn't allowed in a read-only routine"), "{e}"),
                    Ok(_) => panic!("a read-only routine was handed a sign-in"),
                }
            }
        }
        assert!(!asked("gh") && !asked("git"));
        // The host it named: the made-up CLI's token.
        #[cfg(unix)]
        {
            assert_eq!(call("gh-ish", "get", &json!({"host": "signin-jshost.example"})).unwrap(), json!({"token": "gho_madeup777", "from": "gh"}));
            assert!(asked("gh") && !asked("git"));
            // Kept: the CLI's new token isn't asked for until `{fresh: true}`, whose answer is then the one kept.
            fake("gh", "echo gho_madeup888");
            assert_eq!(call("gh-ish", "get", &json!({"host": "signin-jshost.example"})).unwrap()["token"], "gho_madeup777");
            assert_eq!(call("gh-ish", "fresh", &json!({"host": "signin-jshost.example"})).unwrap()["token"], "gho_madeup888");
            assert_eq!(call("gh-ish", "get", &json!({"host": "signin-jshost.example"})).unwrap()["token"], "gho_madeup888");
        }
        #[cfg(not(unix))]
        assert_eq!(call("gh-ish", "get", &json!({"host": "signin-jshost.example"})).unwrap(), Value::Null);
        plugins::signin::FAKE_PROGRAMS.lock().unwrap().clear();
        plugins::remove("gh-ish", false).unwrap();
        sync();
    }

    /// A small http server on this computer: `/clip` answers bytes that aren't UTF-8, `/text` a word, any other path 404.
    fn serve_clips() -> u16 {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in listener.incoming().flatten() {
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).into_owned();
                let (code, body): (&str, Vec<u8>) = match head.split_whitespace().nth(1).unwrap_or("") {
                    "/echo" => ("200 OK", head.split_once("\r\n\r\n").map(|(_, b)| b.as_bytes().to_vec()).unwrap_or_default()),
                    "/clip" => ("200 OK", vec![0xff, 0xfb, 0x90, 0x00, 0x80, 0xfe]),
                    "/text" => ("200 OK", b"hello".to_vec()),
                    _ => ("404 Not Found", b"nope".to_vec()),
                };
                let _ = write!(s, "HTTP/1.1 {code}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
                let _ = s.write_all(&body);
            }
        });
        port
    }

    #[test]
    fn a_plugin_reaches_only_the_addresses_the_user_listed_and_saves_what_it_downloads() {
        let (_env, root) = paths::testenv::folders("jshost-net");
        let mk = root.join("market");
        let d = mk.join("plugins/fetcher");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("echo-plugin.json"),
            json!({"id": "fetcher", "name": "Fetcher", "version": "0.1.0", "main": "main.js",
                   "uses": {"network": {"hosts_from": ["tts"]}},
                   "settings": [{"key": "tts", "label": "TTS server", "type": "string"}]})
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              ctx.command('ping', () => 'pong')
              ctx.command('get', async (a) => await ctx.http.fetch(a.url, a.opts || {}))
              ctx.command('save', async (a) => await ctx.http.fetch(a.url, { save: a.to }))
              ctx.command('widen', () => ctx.settings.set('tts', 'https://collector.acme.example'))
              ctx.command('scribble', (a) => ctx.store.write(a.to, '{"tts":"https://collector.acme.example"}'))
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "fetcher", "source": "./plugins/fetcher"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review("fetcher", Some(&name)).unwrap().uses;
        assert!(agreed.iter().any(|l| l.contains("the addresses you list in its setting “tts”")), "the sheet names the setting: {agreed:?}");
        plugins::install("fetcher", Some(&name), &agreed).unwrap();
        sync();
        let up = (0..50).find_map(|_| call("fetcher", "ping", &json!({})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(up, Some(json!("pong")));

        let port = serve_clips();
        let at = |path: &str| json!({"url": format!("http://127.0.0.1:{port}{path}")});
        let refused = |r: Result<Value>| r.unwrap_err().to_string();
        // Until the user enters an address, the plugin reaches nothing.
        assert!(refused(call("fetcher", "get", &at("/text"))).contains("may reach only"));
        // An address that isn't allowed is refused where it is saved, and nothing is kept.
        assert!(plugins::set_setting("fetcher", "tts", &json!("http://example.com")).unwrap_err().to_string().contains("isn't an address the plugin may reach"));
        assert!(refused(call("fetcher", "get", &at("/text"))).contains("may reach only"));
        // The user's own addresses (plain http, wildcards): reached, and only what they match.
        plugins::set_setting("fetcher", "tts", &json!(format!("http://192.168.*.*:*, http://127.0.0.*:{port}"))).unwrap();
        assert_eq!(call("fetcher", "get", &at("/text")).unwrap(), json!({"status": 200, "body": "hello"}));
        assert!(refused(call("fetcher", "get", &json!({"url": format!("http://127.0.0.1:{}/text", port.wrapping_add(1))}))).contains("may reach only"), "another port");
        assert!(refused(call("fetcher", "get", &json!({"url": format!("http://127.0.0.1:{port}.evil.example/text")}))).contains("may reach only"), "a longer name");
        assert!(refused(call("fetcher", "get", &json!({"url": format!("http://127.000.0.1:{port}/text")}))).contains("may reach only"), "a number curl reads as octal");
        assert!(refused(call("fetcher", "get", &json!({"url": "https://example.com/"}))).contains("may reach only"), "a host nobody named");
        // The plugin can't widen what the user listed: not through its settings, not by writing their file.
        assert!(refused(call("fetcher", "widen", &json!({}))).contains("only you set the addresses"));
        for to in ["settings.json", "./Settings.json", "settings.json."] {
            assert!(refused(call("fetcher", "scribble", &json!({"to": to}))).contains("isn't its to write"), "{to}");
            assert!(refused(call("fetcher", "save", &json!({"url": format!("http://127.0.0.1:{port}/text"), "to": to}))).contains("isn't its to write"), "{to}");
        }
        assert_eq!(plugins::setting("fetcher", "tts"), Some(json!(format!("http://192.168.*.*:*, http://127.0.0.*:{port}"))));
        // Nor under a name the file system folds onto `settings.json` (ſ is s on a Mac): the addresses aren't in the store.
        let _ = call("fetcher", "scribble", &json!({"to": "\u{17f}ettings.json"}));
        assert!(refused(call("fetcher", "get", &json!({"url": "https://collector.acme.example/"}))).contains("may reach only"));
        // A body is sent as written (never `@file` read from disk); NUL, line breaks and `@` header names are refused.
        let echo = |opts: Value| call("fetcher", "get", &json!({"url": format!("http://127.0.0.1:{port}/echo"), "opts": opts}));
        assert_eq!(echo(json!({"method": "POST", "body": "@/etc/hosts"})).unwrap()["body"], json!("@/etc/hosts"));
        for bad in [json!({"method": "POST", "body": "a\u{0}b"}), json!({"headers": {"@/etc/hosts": "x"}}), json!({"headers": {"x": "a\nb"}})] {
            assert!(echo(bad.clone()).is_err(), "{bad}");
        }
        // `save`: the bytes land in the plugin's store as they came (not as text); a failure leaves no file.
        let saved = call("fetcher", "save", &json!({"url": format!("http://127.0.0.1:{port}/clip"), "to": "audio/a.mp3"})).unwrap();
        assert_eq!(saved, json!({"status": 200, "body": "", "saved": "audio/a.mp3", "bytes": 6}));
        assert_eq!(std::fs::read(plugins::data_dir("fetcher").join("audio/a.mp3")).unwrap(), vec![0xff, 0xfb, 0x90, 0x00, 0x80, 0xfe]);
        let gone = call("fetcher", "save", &json!({"url": format!("http://127.0.0.1:{port}/gone"), "to": "audio/b.mp3"})).unwrap();
        assert_eq!(gone, json!({"status": 404, "body": "nope"}));
        let left: Vec<_> = std::fs::read_dir(plugins::data_dir("fetcher").join("audio")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["a.mp3"], "no b.mp3 and no half-written .part");
        assert!(refused(call("fetcher", "save", &json!({"url": format!("http://127.0.0.1:{port}/clip"), "to": "../x.mp3"}))).contains("inside the plugin"));
        assert!(refused(call("fetcher", "save", &json!({"url": format!("http://127.0.0.1:{port}/clip"), "to": "audio/CON.mp3"}))).contains("device name"));
        // A save onto a folder fails, and leaves no half-written file.
        std::fs::create_dir_all(plugins::data_dir("fetcher").join("audio/dir")).unwrap();
        assert!(call("fetcher", "save", &json!({"url": format!("http://127.0.0.1:{port}/clip"), "to": "audio/dir"})).is_err());
        let left: Vec<_> = std::fs::read_dir(plugins::data_dir("fetcher").join("audio")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(!left.iter().any(|n| n.ends_with(".part")), "{left:?}");
        plugins::remove("fetcher", false).unwrap();
        sync();
    }

    #[test]
    fn a_host_half_starting_trims_its_cache_and_leftover_downloads() {
        let (_env, _root) = paths::testenv::folders("jshost-sweep");
        let store = plugins::data_dir("sweeper");
        let ago = |rel: &str, secs: u64| {
            let f = store.join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(&f, "x").unwrap();
            std::fs::File::options().write(true).open(&f).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(secs)).unwrap();
        };
        const DAY: u64 = 86_400;
        ago("cache/old.mp3", 8 * DAY);
        ago("cache/deep/old.png", 8 * DAY);
        ago("cache/new.mp3", DAY);
        ago("audio/b.mp3.1-2.part", 7200);
        ago("audio/c.mp3.1-3.part", 10);
        ago("audio/old.mp3", 30 * DAY);
        ago("notes.txt", 30 * DAY);
        ago("book/chapter.part", 7200);
        sweep_store("sweeper");
        let mut left: Vec<String> = Vec::new();
        fn walk(d: &Path, base: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                if e.path().is_dir() {
                    walk(&e.path(), base, out);
                } else {
                    out.push(e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        walk(&store, &store, &mut left);
        left.sort();
        assert_eq!(left, ["audio/c.mp3.1-3.part", "audio/old.mp3", "book/chapter.part", "cache/new.mp3", "notes.txt"], "the plugin's own .part is its own");
    }

    #[test]
    fn a_plugin_hears_only_the_events_it_asked_for() {
        let (_env, root) = paths::testenv::folders("jshost-events");
        let mk = root.join("market");
        let main = r#"
            export function apply(ctx) {
              const heard = []
              let refused = ''
              ctx.on('minutes_written', (p) => { heard.push(p.call) }).catch((e) => { refused = String(e.message || e) })
              ctx.command('heard', () => ({ heard, refused }))
            }
        "#;
        let mut entries = vec![];
        for (id, uses) in [("hears", json!({"events": {"on": ["minutes_written"]}})), ("deaf", json!({}))] {
            let d = mk.join("plugins").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "main": "main.js", "uses": uses}).to_string()).unwrap();
            std::fs::write(d.join("main.js"), main).unwrap();
            entries.push(json!({"id": id, "source": format!("./plugins/{id}")}));
        }
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": entries}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        for id in ["hears", "deaf"] {
            let agreed = plugins::review(id, Some(&name)).unwrap().uses;
            plugins::install(id, Some(&name), &agreed).unwrap();
        }
        sync();
        let ready = |id: &str| (0..50).any(|_| call(id, "heard", &json!({})).is_ok() || { std::thread::sleep(Duration::from_millis(50)); false });
        assert!(ready("hears") && ready("deaf"), "{:?} {:?}", call("hears", "heard", &json!({})), call("deaf", "heard", &json!({})));
        event("minutes_written", &json!({"id": "call:2026-11-06_100000-acme", "call": "2026-11-06_100000-acme", "title": "Acme"}));
        let got = (0..50).find_map(|_| {
            let v = call("hears", "heard", &json!({})).unwrap();
            if v["heard"].as_array().is_some_and(|a| !a.is_empty()) { Some(v) } else { std::thread::sleep(Duration::from_millis(50)); None }
        });
        assert_eq!(got.unwrap()["heard"], json!(["2026-11-06_100000-acme"]));
        let deaf = call("deaf", "heard", &json!({})).unwrap();
        assert_eq!(deaf["heard"], json!([]), "not asked: not delivered");
        assert!(deaf["refused"].as_str().unwrap().contains("didn't ask to hear"), "{deaf}");
        assert!(plugins::review("hears", Some(&name)).unwrap().uses.iter().any(|l| l.contains("Knows when a call's minutes are written")), "the sheet says it");
        for id in ["hears", "deaf"] {
            plugins::remove(id, false).unwrap();
        }
        sync();
    }

    #[test]
    fn a_plugins_work_rows_come_by_scope_from_its_last_answer_and_go_with_it() {
        let (_env, root) = paths::testenv::folders("jshost-work");
        let alpha = crate::projects::create("Alpha", None, None).unwrap().id;
        let mk = root.join("market");
        let d = mk.join("plugins/gh");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("icon.svg"), "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "gh", "name": "GitHub", "version": "0.1.0", "main": "main.js", "icon": "icon.svg", "uses": {"work": {}}}).to_string()).unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              let broken = false
              ctx.work.provide(({ scope, thread }) => {
                if (broken) throw new Error('GitHub is down')
                const review = { id: 'pr-7', title: 'Review: fix login', kind: 'review', where: 'acme-test/api', url: 'https://git.acme.example/pr/7', thread: 'alpha' }
                return scope === 'me'
                  ? [{ id: 'is-3', title: 'Flaky test', kind: 'assigned', due: '2099-01-01' }, review]
                  : [{ id: 'pr-8', title: 'Bump deps', kind: 'PR', where: 'acme-test/api' }, review]
              })
              ctx.command('break', () => { broken = true; ctx.work.changed(); return 1 })
              ctx.command('sneak', () => ctx.store.write('work.json', '{}'))
              ctx.command('unsneak', () => ctx.store.remove('./work.json'))
              ctx.command('wipe', () => ctx.store.remove(''))
              ctx.command('hang', () => new Promise(() => {}))
            }
            "#,
        )
        .unwrap();
        let q = mk.join("plugins/quiet");
        std::fs::create_dir_all(&q).unwrap();
        std::fs::write(q.join("echo-plugin.json"), json!({"id": "quiet", "name": "Quiet", "version": "0.1.0", "main": "main.js"}).to_string()).unwrap();
        std::fs::write(q.join("main.js"), "export function apply(ctx) { ctx.command('try', () => ctx.work.provide(() => [])) }").unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "gh", "source": "./plugins/gh"}, {"id": "quiet", "source": "./plugins/quiet"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        for id in ["gh", "quiet"] {
            let review = plugins::review(id, Some(&name)).unwrap();
            plugins::install(id, Some(&name), &review.uses).unwrap();
        }
        assert!(plugins::review("gh", Some(&name)).unwrap().uses.iter().any(|l| l == "Adds its items to Today → My work and to threads' Work"));
        sync();
        assert!((0..50).any(|_| call("quiet", "try", &json!({})).is_err_and(|e| e.to_string().contains("didn't ask for “work”")) || { std::thread::sleep(Duration::from_millis(50)); false }), "only with uses.work");
        assert!(call("gh", "sneak", &json!({})).unwrap_err().to_string().contains("work.json"), "its work.json is Echo's to write");
        assert!(call("gh", "unsneak", &json!({})).unwrap_err().to_string().contains("work.json"), "and to remove");
        assert!(call("gh", "wipe", &json!({})).unwrap_err().to_string().contains("not the folder itself"), "the store with it");
        // Never waited for: the first read asks in the background — waited for (`pending`) until it answers.
        let (first, pending) = work::view(None);
        assert!(first.is_empty() && pending, "{first:?}");
        let me = (0..100).map(|_| { std::thread::sleep(Duration::from_millis(50)); work::view(None) }).find(|(r, p)| r.len() == 2 && !p).expect("answered").0;
        assert_eq!((me[0]["id"].as_str(), me[1]["id"].as_str()), (Some("is-3"), Some("pr-7")), "by date, no date last");
        assert_eq!((me[0]["source"].as_str(), me[0]["source_name"].as_str(), me[0]["icon"].as_str()), (Some("gh"), Some("GitHub"), Some("plugin/gh/icon.svg")));
        // Alpha's Work: its own rows and the user's that name it, each once.
        let _ = work::of_thread(&alpha);
        let th = (0..100).map(|_| { std::thread::sleep(Duration::from_millis(50)); work::thread_view(&alpha) }).find(|(r, p)| r.len() == 2 && !p).expect("answered").0;
        assert_eq!(th.iter().map(|r| r["id"].as_str().unwrap()).collect::<Vec<_>>(), ["pr-8", "pr-7"]);
        // It fails: its last rows kept, marked not updated, with why — and where (My work's, not a thread's).
        call("gh", "break", &json!({})).unwrap();
        assert!(work::view(None).1, "changed: waited for again");
        let gone = (0..100).find(|_| { std::thread::sleep(Duration::from_millis(50)); work::error("gh").is_some() });
        assert!(gone.is_some(), "{:?}", work::error("gh"));
        let (place, why) = work::error("gh").unwrap();
        assert!(place == "My work" && why.contains("GitHub is down"), "{place}: {why}");
        let stale = work::rows(None);
        assert_eq!(stale.len(), 2, "kept");
        assert!(stale.iter().all(|r| r["stale"].as_str().is_some_and(|e| e.contains("GitHub is down"))), "{stale:?}");
        // Alpha deleted: its scope goes from the plugin's work.json.
        assert!(std::fs::read_to_string(plugins::data_dir("gh").join(work::FILE)).unwrap().contains(&format!("thread:{alpha}")));
        crate::projects::delete(&alpha).unwrap();
        assert!(!std::fs::read_to_string(plugins::data_dir("gh").join(work::FILE)).unwrap().contains(&format!("thread:{alpha}")));
        // A command that never settles: its caller stops waiting and the host keeps nothing for it.
        assert!(call_within("gh", "hang", &json!({}), Duration::from_millis(200)).unwrap_err().to_string().contains("didn't answer"));
        let left = (0..50).map(|_| { std::thread::sleep(Duration::from_millis(50)); WAITING.lock().unwrap().get("gh").copied() }).find(|w| *w == Some(0));
        assert_eq!(left, Some(Some(0)), "its reply dropped");
        // Removed: its rows go.
        plugins::remove("gh", true).unwrap();
        assert!(work::rows(None).is_empty() && work::of_thread(&alpha).is_empty());
        plugins::remove("quiet", false).unwrap();
        sync();
    }

    #[test]
    fn a_plugin_that_says_its_rows_changed_all_the_time_is_asked_once_a_turn() {
        let (_env, root) = paths::testenv::folders("jshost-work-gap");
        let mk = root.join("market");
        let d = mk.join("plugins/chatty");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "chatty", "name": "Chatty", "version": "0.1.0", "main": "main.js", "uses": {"work": {}}}).to_string()).unwrap();
        // Its provider says changed() itself (not heard), and a command says it twenty times at once.
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              const asks = []
              ctx.work.provide(() => { asks.push(Date.now()); ctx.work.changed(); return [{ id: 'a', title: 'A', kind: 'PR' }] })
              ctx.command('asks', () => asks)
              ctx.command('poke', () => { for (let i = 0; i < 20; i++) ctx.work.changed(); return 1 })
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "chatty", "source": "./plugins/chatty"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let review = plugins::review("chatty", Some(&name)).unwrap();
        plugins::install("chatty", Some(&name), &review.uses).unwrap();
        sync();
        let asks = || call("chatty", "asks", &json!({})).unwrap().as_array().unwrap().iter().map(|t| t.as_f64().unwrap()).collect::<Vec<_>>();
        let _ = work::view(None);
        assert!((0..100).any(|_| { std::thread::sleep(Duration::from_millis(50)); !work::view(None).1 }), "answered, and no longer waited for");
        // Its own changed() inside the provider: not heard — no ask after the first.
        std::thread::sleep(Duration::from_millis(2500));
        assert_eq!(asks().len(), 1, "a provider's own changed() asks nothing more");
        assert!(!work::view(None).1, "and Today's wait ends");
        // Twenty changes at once: asked now, and once more in its next turn (a second on, in tests) for any said after
        // that ask began — never back to back, then quiet.
        call("chatty", "poke", &json!({})).unwrap();
        std::thread::sleep(Duration::from_millis(3000));
        let t = asks();
        assert!((2..=3).contains(&t.len()), "{t:?}");
        assert!(t.windows(2).all(|w| w[1] - w[0] >= 950.0), "a turn between asks: {t:?}");
        std::thread::sleep(Duration::from_millis(2000));
        assert_eq!(asks().len(), t.len(), "then quiet");
        plugins::remove("chatty", true).unwrap();
        sync();
    }

    #[test]
    fn a_plugin_keeps_values_per_thread_and_hears_when_a_thread_goes() {
        let (_env, root) = paths::testenv::folders("jshost-threads");
        let alpha = crate::projects::create("Alpha", None, None).unwrap().id;
        let beta = crate::projects::create("Beta", None, None).unwrap().id;
        let mk = root.join("market");
        let d = mk.join("plugins/repos");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "repos", "name": "Repos", "version": "0.1.0", "main": "main.js", "uses": {"events": {"on": ["threads_changed"]}}}).to_string()).unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              const heard = []
              ctx.on('threads_changed', (p) => { heard.push(p) })
              ctx.command('map', (a) => { ctx.threads.set(a.thread, 'repos', a.repos); return ctx.threads.get(a.thread, 'repos') })
              ctx.command('get', (a) => ctx.threads.get(a.thread, 'repos'))
              ctx.command('list', () => ctx.threads.list('repos'))
              ctx.command('sneak', () => ctx.store.write('threads.json', '{}'))
              ctx.command('wipe', () => ctx.store.remove('./threads.json'))
              ctx.command('wipe-all', (a) => ctx.store.remove(a.path))
              ctx.command('heard', () => heard)
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "repos", "source": "./plugins/repos"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let review = plugins::review("repos", Some(&name)).unwrap();
        assert!(review.uses.iter().any(|l| l.contains("Knows when a thread is renamed, merged or deleted")), "the sheet says it: {:?}", review.uses);
        plugins::install("repos", Some(&name), &review.uses).unwrap();
        sync();
        let mapped = (0..50).find_map(|_| call("repos", "map", &json!({"thread": alpha, "repos": ["acme-test/api"]})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(mapped.unwrap(), json!(["acme-test/api"]));
        assert_eq!(call("repos", "get", &json!({"thread": beta})).unwrap(), Value::Null, "Alpha's only");
        assert_eq!(call("repos", "list", &json!({})).unwrap(), json!([{"thread": alpha, "value": ["acme-test/api"]}]));
        assert!(call("repos", "map", &json!({"thread": "nope", "repos": []})).unwrap_err().to_string().contains("no thread"));
        assert!(call("repos", "sneak", &json!({})).unwrap_err().to_string().contains("ctx.threads.set"), "its threads.json is Echo's to write");
        assert!(call("repos", "wipe", &json!({})).unwrap_err().to_string().contains("ctx.threads.set"), "…and not its to remove");
        for path in ["", "./", "././"] {
            let err = call("repos", "wipe-all", &json!({"path": path})).unwrap_err().to_string();
            assert!(err.contains("not the folder itself"), "{path:?}: {err}");
        }
        assert!(plugins::data_dir("repos").join("threads.json").is_file(), "the store and Echo's files in it kept");
        // A read-only routine reads them, never sets them.
        std::env::set_var(crate::guard::RUN_MODE, "routine");
        let refused = call("repos", "map", &json!({"thread": beta, "repos": ["acme-test/web"]}));
        let read = call("repos", "get", &json!({"thread": alpha}));
        std::env::remove_var(crate::guard::RUN_MODE);
        assert!(refused.unwrap_err().to_string().contains("read-only routine"));
        assert_eq!(read.unwrap(), json!(["acme-test/api"]));
        // A host that isn't the app's (this one, until it is marked): it hears nothing and takes nothing queued.
        let born = |id: &str| json!(crate::projects::load(id).unwrap().stages[0].at);
        let (alpha_born, beta_born) = (born(&alpha), born(&beta));
        assert!(!hosts_here());
        crate::projects::update(&beta, |p| {
            p.name = "Beta 2".into();
            Ok(())
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(1300)); // over a tick
        assert!(paths::state_dir().join("plugin-events.jsonl").is_file(), "left for the app");
        assert_eq!(call("repos", "heard", &json!({})).unwrap(), json!([]));
        as_app();
        let heard = (0..60).find_map(|_| {
            let v = call("repos", "heard", &json!({})).unwrap();
            if v.as_array().is_some_and(|a| a.len() == 1) { Some(v) } else { std::thread::sleep(Duration::from_millis(50)); None }
        });
        assert_eq!(heard.unwrap(), json!([{"thread": beta, "born": beta_born, "change": "renamed", "name": "Beta 2", "from": "Beta"}]), "the app takes it");
        // Alpha renamed, then deleted: the plugin hears both; its value for Alpha is gone.
        crate::projects::update(&alpha, |p| {
            p.name = "Alpha 2".into();
            Ok(())
        })
        .unwrap();
        crate::projects::delete(&alpha).unwrap();
        let heard = (0..50).find_map(|_| {
            let v = call("repos", "heard", &json!({})).unwrap();
            if v.as_array().is_some_and(|a| a.len() == 3) { Some(v) } else { std::thread::sleep(Duration::from_millis(50)); None }
        });
        assert_eq!(
            heard.unwrap().as_array().unwrap()[1..],
            [
                json!({"thread": alpha, "born": alpha_born, "change": "renamed", "name": "Alpha 2", "from": "Alpha"}),
                json!({"thread": alpha, "born": alpha_born, "change": "deleted", "values": {"repos": ["acme-test/api"]}})
            ],
            "the deleted thread's values come with it, the plugin's own"
        );
        assert_eq!(call("repos", "get", &json!({"thread": alpha})).unwrap(), Value::Null);
        assert_eq!(call("repos", "list", &json!({})).unwrap(), json!([]));
        // A terminal's delete (no host there) left in the file for the app: heard within a tick, its own part only.
        let line = json!({"event": "threads_changed", "payload": {"thread": beta, "change": "deleted", "values": {}}, "each": {"repos": {"values": {"repos": 1}}, "other": {"values": {"x": 2}}}});
        std::fs::write(paths::state_dir().join("plugin-events.jsonl"), format!("{line}\n")).unwrap();
        let heard = (0..60).find_map(|_| {
            let v = call("repos", "heard", &json!({})).unwrap();
            if v.as_array().is_some_and(|a| a.len() == 4) { Some(v) } else { std::thread::sleep(Duration::from_millis(50)); None }
        });
        assert_eq!(heard.unwrap()[3], json!({"thread": beta, "change": "deleted", "values": {"repos": 1}}));
        assert!(!paths::state_dir().join("plugin-events.jsonl").exists(), "taken");
        APP.store(false, Ordering::Relaxed); // the other tests' processes aren't the app
        plugins::remove("repos", false).unwrap();
        sync();
    }

    #[test]
    fn a_plugin_changes_files_only_in_folders_it_was_given_and_only_if_it_asked() {
        let (_env, root) = paths::testenv::folders("jshost-fs-write");
        let vault = root.join("vault");
        std::fs::create_dir_all(vault.join("People")).unwrap();
        std::fs::write(vault.join("People/Dana.md"), "# Dana").unwrap();
        std::fs::write(vault.join("Old.md"), "old").unwrap();
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.md"), "no").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, vault.join("out")).unwrap();
            std::os::unix::fs::symlink(outside.join("secret.md"), vault.join("Secret.md")).unwrap();
        }
        let mk = root.join("market");
        let main = r#"
            export function apply(ctx) {
              const r = () => ctx.fs.folders()[0]
              ctx.command('write', (a) => ctx.fs.write(r(), a.path, a.text))
              ctx.command('move', (a) => ctx.fs.move(r(), a.from, a.to))
              ctx.command('remove', (a) => ctx.fs.remove(r(), a.path))
            }
        "#;
        for (id, folders) in [("writer", json!({"what": "vaults", "write": true})), ("reader", json!({"what": "vaults"}))] {
            let d = mk.join("plugins").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "main": "main.js", "uses": {"folders": folders}}).to_string()).unwrap();
            std::fs::write(d.join("main.js"), main).unwrap();
        }
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "writer", "source": "./plugins/writer"}, {"id": "reader", "source": "./plugins/reader"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let sheet = plugins::review("writer", Some(&name)).unwrap().uses;
        assert!(sheet.iter().any(|l| l.contains("Reads and changes the vaults you pick") && l.contains("write, move and delete")), "{sheet:?}");
        for id in ["writer", "reader"] {
            let agreed = plugins::review(id, Some(&name)).unwrap().uses;
            plugins::install(id, Some(&name), &agreed).unwrap();
            crate::grants::grant(id, &vault).unwrap();
        }
        sync();
        let run = |id: &str, cmd: &str, a: Value| {
            for _ in 0..50 {
                match call(id, cmd, &a) {
                    Err(e) if e.to_string().contains("no command") => std::thread::sleep(Duration::from_millis(50)),
                    r => return r.map_err(|e| e.to_string()),
                }
            }
            Err("never loaded".to_string())
        };
        // Write (a new folder too), overwrite, move, delete — inside the vault.
        run("writer", "write", json!({"path": "Notes/New.md", "text": "# New"})).unwrap();
        assert_eq!(std::fs::read_to_string(vault.join("Notes/New.md")).unwrap(), "# New");
        run("writer", "write", json!({"path": "People/Dana.md", "text": "# Dana Lee"})).unwrap();
        assert_eq!(std::fs::read_to_string(vault.join("People/Dana.md")).unwrap(), "# Dana Lee");
        run("writer", "move", json!({"from": "Old.md", "to": "Archive/Old.md"})).unwrap();
        assert!(!vault.join("Old.md").exists() && vault.join("Archive/Old.md").is_file());
        assert!(run("writer", "move", json!({"from": "Notes/New.md", "to": "People/Dana.md"})).unwrap_err().contains("there already"), "never over another file");
        run("writer", "remove", json!({"path": "Archive/Old.md"})).unwrap();
        run("writer", "remove", json!({"path": "Archive"})).unwrap();
        assert!(!vault.join("Archive").exists());
        assert!(run("writer", "remove", json!({"path": "People"})).unwrap_err().contains("files in it"));
        // Never out of the vault: `..`, a link to a folder outside, a link to a file outside, the vault itself.
        assert!(run("writer", "write", json!({"path": "../outside/x.md", "text": "x"})).is_err());
        assert!(run("writer", "remove", json!({"path": ""})).is_err());
        #[cfg(unix)]
        {
            assert!(run("writer", "write", json!({"path": "out/x.md", "text": "x"})).unwrap_err().contains("leads outside"));
            assert!(run("writer", "write", json!({"path": "out/deeper/x.md", "text": "x"})).unwrap_err().contains("leads outside"));
            let e = run("writer", "write", json!({"path": "Secret.md", "text": "x"})).unwrap_err();
            assert!(e.contains("is a link") || e.contains("leads outside"), "{e}");
            let e = run("writer", "remove", json!({"path": "Secret.md"})).unwrap_err();
            assert!(e.contains("is a link") || e.contains("leads outside"), "{e}");
            let e = run("writer", "move", json!({"from": "Secret.md", "to": "Mine.md"})).unwrap_err();
            assert!(e.contains("is a link") || e.contains("leads outside"), "{e}");
            // A link to a file inside the vault isn't changed through either.
            std::os::unix::fs::symlink(vault.join("People/Dana.md"), vault.join("Alias.md")).unwrap();
            assert!(run("writer", "write", json!({"path": "Alias.md", "text": "x"})).unwrap_err().contains("is a link"));
            assert_eq!(std::fs::read_to_string(outside.join("secret.md")).unwrap(), "no");
            assert!(!outside.join("x.md").exists() && !outside.join("deeper").exists());
        }
        // Hidden files (other programs' settings and code) and Echo's own folders are never written.
        std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
        assert!(run("writer", "write", json!({"path": ".obsidian/plugins/x/main.js", "text": "x"})).unwrap_err().contains("is hidden"));
        assert!(run("writer", "write", json!({"path": "Notes/.git/hooks/pre-commit", "text": "x"})).unwrap_err().contains("is hidden"));
        assert!(run("writer", "write", json!({"path": "Notes...", "text": "x"})).is_err(), "a name ending in a dot");
        // A hidden folder by another name is hidden too: a link inside the vault to it, or Windows' short name of it.
        std::fs::write(vault.join(".obsidian/app.json"), "{}").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(vault.join(".obsidian"), vault.join("settings")).unwrap();
            assert!(run("writer", "write", json!({"path": "settings/x.json", "text": "x"})).unwrap_err().contains("is hidden"));
            assert!(run("writer", "remove", json!({"path": "settings/app.json"})).unwrap_err().contains("is hidden"));
        }
        #[cfg(windows)]
        {
            // `C:\…\OBSIDI~1`: its 8.3 name, when the volume makes them (else nothing to check).
            let short = |p: &std::path::Path| {
                use std::os::windows::process::CommandExt;
                let o = std::process::Command::new("cmd").arg("/c").raw_arg(format!("for %I in (\"{}\") do @echo %~snxI", p.display())).output().ok()?;
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty() && !s.starts_with('.'))
            };
            if let Some(s) = short(&vault.join(".obsidian")) {
                assert!(run("writer", "write", json!({"path": format!("{s}/x.json"), "text": "x"})).unwrap_err().contains("is hidden"), "{s}");
                assert!(run("writer", "remove", json!({"path": format!("{s}/app.json")})).unwrap_err().contains("is hidden"));
                assert!(run("writer", "move", json!({"from": format!("{s}/app.json"), "to": "Stolen.json"})).unwrap_err().contains("is hidden"));
                assert!(run("writer", "remove", json!({"path": s})).unwrap_err().contains("is hidden"), "the folder itself");
            }
            // A file written by its short name keeps its own long name.
            std::fs::write(vault.join("averylongnotename.md"), "long").unwrap();
            if let Some(s) = short(&vault.join("averylongnotename.md")).filter(|s| s != "averylongnotename.md") {
                run("writer", "write", json!({"path": s, "text": "new"})).unwrap();
                assert_eq!(std::fs::read_to_string(vault.join("averylongnotename.md")).unwrap(), "new");
                assert!(!std::fs::read_dir(&vault).unwrap().flatten().any(|e| e.file_name().to_string_lossy() == s), "no file named {s}");
            }
            // `:` names a data stream: refused, and nothing left behind.
            assert!(run("writer", "write", json!({"path": "People/X.md:ads", "text": "x"})).unwrap_err().contains("isn't a file name"));
            assert!(!vault.join("People/.X.md").exists() && !vault.join("People/X.md").exists());
        }
        assert_eq!(std::fs::read_to_string(vault.join(".obsidian/app.json")).unwrap(), "{}");
        assert!(crate::grants::grant("writer", &paths::config_dir()).unwrap_err().to_string().contains("Echo's own files"));
        assert!(crate::grants::grant("writer", &paths::home()).unwrap_err().to_string().contains("Echo's own files"), "a folder holding the data folder's .echo");
        // A plugin that only reads can't.
        assert!(run("reader", "write", json!({"path": "Notes/R.md", "text": "x"})).unwrap_err().contains("folders.write"));
        assert!(!vault.join("Notes/R.md").exists());
        for id in ["writer", "reader"] {
            plugins::remove(id, false).unwrap();
        }
        sync();
    }

    // A release build has no ECHOCTL_PICK_FOLDER: there `pickFolder` would open the real folder picker.
    #[cfg(debug_assertions)]
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
        let uses = json!({"folders": {"what": "vaults"}, "sessions": {"skills": true, "read": true, "hints": true, "commands": ["search", "backlinks"]}, "echo": {"read": ["clients"]}});
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
        assert!(agreed.iter().any(|l| l.contains("lets Claude in Echo run its commands “search” and “backlinks” without asking")), "{agreed:?}");
        plugins::install("vaults", Some(&name), &agreed).unwrap();
        assert_eq!(plugins::session_commands(), vec![("vaults".to_string(), "search".to_string()), ("vaults".to_string(), "backlinks".to_string())]);
        sync();
        let pick = EnvVar::set("ECHOCTL_PICK_FOLDER", vault.to_str().unwrap());
        let mut last = String::new();
        let got = (0..50).find_map(|_| match call("vaults", "add", &json!({})) {
            Ok(v) => Some(v),
            Err(e) => {
                last = format!("{e:#}");
                std::thread::sleep(Duration::from_millis(50));
                None
            }
        });
        drop(pick);
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
        let mut files: Vec<_> = std::fs::read_dir(&sp[0]).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        files.sort();
        assert_eq!(files, [".claude-plugin", "scripts", "skills"], "only its manifest, Echo's CLI launcher and the skills");
        // Its skills reach the Echo CLI as Echo's own do: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`, Echo's launcher.
        assert_eq!(std::fs::read_to_string(sp[0].join("scripts/echoctl")).unwrap(), include_str!("../../../plugin/scripts/echoctl"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(sp[0].join("scripts/echoctl")).unwrap().permissions().mode() & 0o111, 0o111);
        }
        assert!(crate::grants::session_clis(&sp).iter().any(|c| c.ends_with("/scripts/echoctl") && c.starts_with(&sp[0].display().to_string())));
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
