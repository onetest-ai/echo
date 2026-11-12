//! Ask's conversations (spec K12, D22, D25, D38, D44): a real Claude Code process per conversation in a knowledge
//! vault, which Echo drives through stream-json, shows and can stop. A process runs §5.4's command on the vault's own
//! profile (`ai::prepare_claude`) in the vault's root, under the guard in `vault` mode (its scope in the process's
//! environment and in its own `--settings` file, D40), and holds the profile's lease while it lives (`ai::ready`,
//! counted in `terminal::holds_profile`): a sign-in of that profile closes it (`signin.rs`), as it closes the panel's.
//!
//! A conversation keeps its events in memory, numbered, across its processes (with no process, idle for `IDLE`, it
//! lets them go and reads them again from its transcript when opened): a mode change ends the process and the next
//! message starts one with `--resume` (tools and the guard's mode are fixed per process, D25), as does a message after
//! an idle stop. The page long-polls `read`, as it does a terminal. One process per conversation: one open in the
//! Claude panel is refused here, and the panel refuses one live here (K12-12); at most `MAX_LIVE` run at once (a new
//! one is refused while all of them are answering) and one idle for `IDLE` stops (D38). Conversations started here
//! join the vault's list (`terminal::remember`) but never become the panel's current one (D44). Everything Claude Code prints is others' words: the parser makes it events
//! (`stream.rs`), text reaches HTML only through the vault-page writer.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::Write as _;
use std::path::Path;
use std::process::{Child, ChildStdin};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::{stream, transcript, Vault};
use crate::{ai, guard, paths, terminal};

mod attach;
mod permits;
mod skills;

pub use attach::{attach_call, Mismatch};

/// At most this many conversations have a process at once (D38); for another, the longest idle stops.
const MAX_LIVE: usize = 3;
/// A process with no turn going, idle this long, stops (the next message resumes it).
const IDLE: Duration = Duration::from_secs(600);
/// Stop: an interrupted turn that hasn't ended this long after has its process ended (K12-09).
const STOP_AFTER: Duration = Duration::from_secs(5);
/// Events kept per conversation (older ones are a transcript's, K12-11).
const KEEP: usize = 5000;
/// At most this many events in one answer to the page (it asks again at once for the rest).
const CHUNK: usize = 500;
/// A message's length (K12-07).
const TEXT_MAX: usize = 20_000;
/// No stdout line for this long: what the render throttle held back is shown (`Parser::tick`).
const TICK: Duration = Duration::from_millis(100);
/// Every mode's tools (§5.4). Never Bash, NotebookEdit or an MCP tool (K12-02); print mode has no TodoWrite (R6).
const READ_TOOLS: [&str; 5] = ["Read", "Grep", "Glob", "Skill", "Agent"];

/// What a conversation may do (spec §5.4): its tools, and the guard's `ECHOCTL_RUN_CHAT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Ask,
    Curate,
    Research,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode> {
        match s {
            "ask" => Ok(Mode::Ask),
            "curate" => Ok(Mode::Curate),
            "research" => Ok(Mode::Research),
            other => bail!("{other:?} isn't a mode of Ask (ask, curate or research)"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::Ask => "ask",
            Mode::Curate => "curate",
            Mode::Research => "research",
        }
    }

    /// `--tools`.
    fn tools(self) -> Vec<&'static str> {
        let mut t = READ_TOOLS.to_vec();
        match self {
            Mode::Ask => {}
            Mode::Curate => t.extend(["Edit", "Write"]),
            Mode::Research => t.extend(["WebSearch", "WebFetch"]),
        }
        t
    }

    /// `--allowed-tools`, the ones that never prompt: Curate's edits and Research's fetched pages are asked about (D24).
    fn allowed(self) -> Vec<&'static str> {
        let mut t = READ_TOOLS.to_vec();
        if self == Mode::Research {
            t.push("WebSearch");
        }
        t
    }

    /// The tools asked about through the host, a card each (D24): Curate's edits, Research's fetched pages.
    fn asked(self) -> Vec<&'static str> {
        let allowed = self.allowed();
        self.tools().into_iter().filter(|t| !allowed.contains(t)).collect()
    }
}

/// How a process takes up its conversation: a new one (its id, and the first message, which names it) or one Claude
/// Code has.
pub enum Session {
    New { id: String, first: String },
    Resume(String),
}

/// What a message is about (K12-07, K12-10): a page of the vault, or a call whose minutes go with it.
pub enum Context {
    Page(String),
    /// A call's folder, found by Echo from the name the page uses (never a path from the page), and whether the user
    /// said *Attach anyway* to a call on another profile (X-P3).
    Call { dir: std::path::PathBuf, anyway: bool },
}

/// What the page gets of a conversation: its events from `from` on, where to ask next, and its state.
#[derive(Debug)]
pub struct ChatChunk {
    pub events: Vec<Value>,
    pub next: u64,
    pub live: bool,
    pub busy: bool,
    pub waiting: bool,
}

/// §5.4's command line for a conversation of `v` on profile `p` (`settings`: its own settings file). Never
/// `--dangerously-skip-permissions`, whatever the folder session's switch says (D24); a managed profile gets the vault's
/// skills as a plugin and its rules (`terminal::vault_args`), and `--setting-sources user` from `ai::prepare_claude`.
fn args(v: &Vault, p: &ai::Profile, mode: Mode, session: &Session, settings: &Path, skills: &skills::Catalog) -> Result<Vec<String>> {
    let mut a: Vec<String> = [
        "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
        "--permission-mode", "default", "--permission-prompts", "host",
        // Hidden, what Claude Code's SDKs pass: without it every ask is refused (the design note's *Chat protocol*).
        "--permission-prompt-tool", "stdio",
        "--prompt-suggestions", "--strict-mcp-config",
    ]
    .map(String::from)
    .to_vec();
    a.extend(["--tools".into(), mode.tools().join(","), "--allowed-tools".into(), mode.allowed().join(",")]);
    a.extend(["--settings".into(), settings.display().to_string()]);
    match session {
        Session::New { id, first } => {
            let first: String = first.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect();
            a.extend(["--session-id".into(), id.clone(), "--name".into(), format!("{}: {}", v.name, first.trim_end())]);
        }
        Session::Resume(id) => a.extend(["--resume".into(), id.clone()]),
    }
    // A local model by its own name, as in the panel's session.
    if let Some(m) = (p.kind == ai::Kind::Local).then(|| p.notes_model()).flatten() {
        a.extend(["--model".into(), m]);
    }
    a.extend(terminal::vault_args(v, p)?);
    let names = serde_json::to_string(&skills.names())?;
    let prompt = format!("You are in an Echo Knowledge conversation. Available Skill tool names from this AI profile's user skills and this vault: {names}. User-level skills and commands are supported directly from their installed locations. Use these exact names. A message starting with /name requests that skill with the remaining text as its arguments. If no skill fits, search the vault with Read, Grep and Glob. The conversation's tools and permissions also apply inside skills and sub-agents; loading a skill does not grant shell access, access to other data or extra write permissions.");
    if let Some(at) = a.iter().position(|x| x == "--append-system-prompt") {
        a[at + 1].push_str(&format!(" {prompt}"));
    } else {
        a.extend(["--append-system-prompt".into(), prompt]);
    }
    Ok(a)
}

/// The same catalog the process and guard use, without host paths or instruction bodies.
pub fn skills_value(v: &Vault) -> Result<Value> {
    let p = ai::Settings::load()?.get(&v.profile)?;
    Ok(json!({"skills": skills::Catalog::load(v, &p)?.0}))
}

/// The guard's scope for a conversation (spec §5.4, ruling R11): every key always set.
fn scope(v: &Vault, mode: Mode, skills: &skills::Catalog) -> Vec<(String, String)> {
    vec![
        (guard::RUN_MODE.into(), "vault".into()),
        (guard::RUN_VAULT.into(), v.root.display().to_string()),
        (guard::RUN_CHAT.into(), mode.name().into()),
        (guard::RUN_SKILLS.into(), skills.names().join("\n")),
        (guard::RUN_READ.into(), skills.read_paths().join("\n")),
        (guard::RUN_SOURCES.into(), v.path("sources").filter(|s| !s.is_empty()).unwrap_or_else(|| "raw_inbox".into())),
    ]
}

/// A conversation: its events and, while it runs, its process.
struct Conv {
    id: String,
    vault: String,
    /// The vault's profile when it started: its transcript is in that profile's Claude Code folder.
    profile: String,
    state: Mutex<State>,
    changed: Condvar,
    /// One message at a time (a start can take seconds: the profile's sign-in check).
    sending: Mutex<()>,
}

struct State {
    /// The events `start..start + events.len()`.
    start: u64,
    events: VecDeque<Value>,
    proc: Option<Proc>,
    /// A turn is going: from a message to its `result`, or to the process's end.
    busy: bool,
    /// Messages so far (a stop's timer is for one turn).
    turn: u64,
    /// Permission requests waiting for the user, by request id.
    pending: BTreeMap<String, Pending>,
    /// The user allowed edits for the rest of the conversation (*always*): Echo answers them itself, as it does
    /// fetches of these domains. The conversation's, so a restart (a mode change) keeps them.
    always_edits: bool,
    always_domains: BTreeSet<String>,
    /// What the turn going asked and edited, for its end.
    this_turn: permits::Turn,
    /// Why files weren't committed, said once in the conversation (X-G4).
    told: BTreeSet<String>,
    /// Claude Code has the conversation (a process of it began its session, or it is from the vault's list): the next
    /// start resumes it.
    started: bool,
    /// It holds the conversation from its beginning: its past was read from Claude Code's transcript (K12-11) — the
    /// events it gave come first —, or nothing came before what it holds. Its events let go (`sweep`), it doesn't.
    history: bool,
    /// A page opened it from before what it holds while a turn went: it is read again once the turn is over.
    due: bool,
    last_active: Instant,
    /// When it was last used, for the list.
    active_at: Option<String>,
    /// A streaming block's latest render (block → its event's number), while the block streams.
    renders: HashMap<String, u64>,
}

/// A permission request waiting for the user.
struct Pending {
    /// The request, its input whole (an allow sends it back as it is).
    request: Value,
    /// An edit's file as Echo read it for the card: its activity line and the turn's commit, once done.
    edit: Option<permits::Edit>,
    /// A `WebFetch`'s domain.
    domain: Option<String>,
}

struct Proc {
    gen: u64,
    pid: u32,
    mode: Mode,
    profile: String,
    skills: skills::Catalog,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    /// Echo ended it: its exit isn't a crash.
    stopped: Arc<AtomicBool>,
    /// Being ended: live until it has exited, no longer counted toward `MAX_LIVE`.
    ending: bool,
}

impl State {
    fn end(&self) -> u64 {
        self.start + self.events.len() as u64
    }

    fn push(&mut self, mut e: Value) {
        let n = self.end();
        e["n"] = json!(n);
        // A streaming block's newer render supersedes the one before, which then keeps no copy of the block (one per
        // render would hold a long answer many times over); its number stays.
        let heavy = match e["type"].as_str() {
            Some("text") => Some("html"),
            Some("thinking") => Some("text"),
            _ => None,
        };
        if let (Some(field), Some(block)) = (heavy, e["block"].as_str().map(str::to_owned)) {
            let before = if e["done"] == true { self.renders.remove(&block) } else { self.renders.insert(block, n) };
            if let Some(old) = before.and_then(|m| m.checked_sub(self.start)).and_then(|i| self.events.get_mut(i as usize)).and_then(Value::as_object_mut) {
                old.remove(field);
                old.insert("superseded".into(), json!(true));
            }
        }
        self.events.push_back(e);
        if self.events.len() > KEEP {
            self.events.pop_front();
            self.start += 1;
        }
    }

    fn running(&self) -> Option<&Proc> {
        self.proc.as_ref().filter(|p| !p.ending)
    }

    fn touch(&mut self) {
        self.last_active = now();
        self.active_at = Some(chrono::Local::now().to_rfc3339());
    }

    fn flags(&self) -> (bool, bool, bool) {
        (self.proc.is_some(), self.busy, !self.pending.is_empty())
    }
}

impl Conv {
    fn new(id: &str, v: &Vault, started: bool) -> Arc<Conv> {
        let state = State {
            start: 0,
            events: VecDeque::new(),
            proc: None,
            busy: false,
            turn: 0,
            pending: BTreeMap::new(),
            always_edits: false,
            always_domains: BTreeSet::new(),
            this_turn: permits::Turn::default(),
            told: BTreeSet::new(),
            started,
            history: false,
            due: false,
            last_active: now(),
            active_at: None,
            renders: HashMap::new(),
        };
        Arc::new(Conv { id: id.into(), vault: v.id.clone(), profile: v.profile.clone(), state: Mutex::new(state), changed: Condvar::new(), sending: Mutex::new(()) })
    }
}

static CHATS: Mutex<BTreeMap<String, Arc<Conv>>> = Mutex::new(BTreeMap::new());
/// One start at a time, so `MAX_LIVE` holds.
static STARTING: Mutex<()> = Mutex::new(());
static GEN: AtomicU64 = AtomicU64::new(1);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn all() -> Vec<Arc<Conv>> {
    lock(&CHATS).values().cloned().collect()
}

#[cfg(test)]
static SKEW: Mutex<Duration> = Mutex::new(Duration::ZERO);

/// The clock of idle stops and a stop's 5 s (a test moves it ahead).
fn now() -> Instant {
    #[cfg(test)]
    return Instant::now() + *lock(&SKEW);
    #[cfg(not(test))]
    Instant::now()
}

fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Conversation `id` of `v`: one Echo holds, or one of the vault's list (started in Ask or in the panel before).
fn conv_of(v: &Vault, id: &str) -> Option<Arc<Conv>> {
    if !valid_id(id) {
        return None;
    }
    let mut chats = lock(&CHATS);
    if let Some(c) = chats.get(id) {
        return (c.vault == v.id && c.profile == v.profile).then(|| c.clone());
    }
    if !terminal::listed(&terminal::vault_store_key(v), id) {
        return None;
    }
    let c = Conv::new(id, v, true);
    chats.insert(id.into(), c.clone());
    Some(c)
}

/// A page the message names, checked as a path of the vault (untrusted-text R4).
fn page_of(v: &Vault, rel: &str) -> Result<String> {
    let rel = rel.trim();
    if !super::page::inside(v, rel).is_ok_and(|p| p.is_file()) {
        bail!("“{rel}” isn't a page of this vault");
    }
    Ok(rel.to_string())
}

/// The user's text as Claude Code reads it: with the pages it is about named, and the minutes of the calls attached
/// after it, each framed as others' words (K12-10).
fn message(text: &str, pages: &[String], calls: &[attach::Attached]) -> String {
    let mut m = text.to_string();
    if !pages.is_empty() {
        let named: Vec<String> = pages.iter().map(|p| format!("`{p}`")).collect();
        m.push_str(&format!("\n\n(The pages this is about: {})", named.join(", ")));
    }
    for c in calls {
        m.push_str("\n\n");
        m.push_str(&c.text);
    }
    m
}

fn write_line(stdin: &Mutex<Option<ChildStdin>>, v: &Value) -> std::io::Result<()> {
    let mut s = lock(stdin);
    let w = s.as_mut().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "its input is closed"))?;
    w.write_all(format!("{v}\n").as_bytes())?;
    w.flush()
}

/// Send `text` to a conversation of vault `vault` (a new one when `conversation` is None), in `mode`, naming the pages
/// of `context` and carrying the minutes of its calls (K12-10; a call on another profile is refused unless the user
/// said *Attach anyway*, and then recorded as a notice): its process is started (or restarted with `--resume` for
/// another mode, D25) and the message written (K12-07). The conversation's id. A sign-in needed is an `error` event of
/// the conversation with the profile's sign-in action (K12-13).
pub fn send(vault: &str, conversation: Option<&str>, text: &str, mode: Mode, context: &[Context]) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        bail!("Write a message first");
    }
    if text.chars().count() > TEXT_MAX {
        bail!("A message holds at most {TEXT_MAX} characters");
    }
    let v = super::get(vault)?;
    if !super::available(&v) {
        bail!("the vault's folder isn't there: {}", v.root.display());
    }
    if mode == Mode::Research && !v.ask.research {
        bail!("Research is off for this vault (its Settings → Ask)");
    }
    let skills = skills::Catalog::load(&v, &ai::Settings::load()?.get(&v.profile)?)?;
    skills.validate(text)?;
    let (mut pages, mut calls) = (vec![], vec![]);
    for x in context {
        match x {
            Context::Page(p) => pages.push(page_of(&v, p)?),
            Context::Call { dir, anyway } => calls.push(attach_call(&v, dir, *anyway)?),
        }
    }
    let c = match conversation {
        Some(id) => conv_of(&v, id).ok_or_else(|| anyhow!("that conversation isn't one of this vault's"))?,
        None => {
            let c = Conv::new(&terminal::new_uuid(), &v, false);
            lock(&CHATS).insert(c.id.clone(), c.clone());
            c
        }
    };
    if terminal::live_session(&c.id) {
        bail!("This conversation is open in the terminal panel — close it there to continue it here");
    }
    let _one = lock(&c.sending);
    if lock(&c.state).busy {
        bail!("Wait for this answer, or stop it");
    }
    open_history(&c, &v, None);
    if let Err(e) = running(&c, &v, mode, text, &skills) {
        let Some(s) = e.downcast_ref::<ai::SignInNeeded>() else {
            if conversation.is_none() {
                lock(&CHATS).remove(&c.id); // a new one that never started is no conversation
            }
            return Err(e);
        };
        let mut st = lock(&c.state);
        st.push(json!({"type": "error", "text": s.to_string(), "action": "signin", "profile": s.profile}));
        c.changed.notify_all();
        return Ok(c.id.clone());
    }
    let stdin = {
        let mut st = lock(&c.state);
        let stdin = st.running().map(|p| p.stdin.clone()).ok_or_else(|| anyhow!("Claude Code stopped — send it again"))?;
        let context: Vec<Value> = pages.iter().map(|p| json!({"page": p})).chain(calls.iter().map(|a| json!({"call": a.call, "title": a.title, "date": a.date}))).collect();
        st.push(json!({"type": "user", "text": text, "context": context}));
        for a in calls.iter().filter_map(|a| a.across.as_ref()) {
            st.push(json!({"type": "notice", "kind": "attached-anyway", "text": a}));
        }
        st.busy = true;
        st.turn += 1;
        st.this_turn = permits::Turn { question: text.to_string(), ..Default::default() };
        st.touch();
        c.changed.notify_all();
        stdin
    };
    // Its reader ends the turn if the process is gone.
    write_line(&stdin, &json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": message(text, &pages, &calls)}]}})).map_err(|e| anyhow!("Claude Code didn't take the message: {e}"))?;
    Ok(c.id.clone())
}

/// A process for `c` in `mode`: the one it has, or a new one once one in another mode has ended (D25).
fn running(c: &Arc<Conv>, v: &Vault, mode: Mode, first: &str, skills: &skills::Catalog) -> Result<()> {
    let mut st = lock(&c.state);
    match st.running() {
        Some(p) if p.mode == mode && p.skills == *skills => return Ok(()),
        Some(_) => {
            end(&mut st);
            c.changed.notify_all();
        }
        None => {}
    }
    let (st, _) = c.changed.wait_timeout_while(st, Duration::from_secs(10), |st| st.proc.is_some()).unwrap_or_else(|e| e.into_inner());
    if st.proc.is_some() {
        bail!("the conversation's Claude Code is still stopping — try again in a moment");
    }
    let resume = st.started;
    drop(st);
    start(c, v, mode, resume, first, skills)
}

/// Start `c`'s process.
fn start(c: &Arc<Conv>, v: &Vault, mode: Mode, resume: bool, first: &str, skills: &skills::Catalog) -> Result<()> {
    // Skills can change since the vault was connected. The prompt, plugin and guard use this same snapshot.
    let mut v = v.clone();
    v.detected.skills = super::detect::skills(&v.root);
    let ready = ai::ready(&v.profile)?;
    let p = ready.profile().clone();
    let settings = super::cache_dir(&v.id).join("chat").join(&c.id).join("settings.json");
    std::fs::create_dir_all(settings.parent().expect("its folder"))?;
    let session = if resume { Session::Resume(c.id.clone()) } else { Session::New { id: c.id.clone(), first: first.into() } };
    let args = args(&v, &p, mode, &session, &settings, skills)?;
    let mut scope = scope(&v, mode, skills);
    // A managed profile's generated vault plugin also has supporting files outside the vault.
    if let Some(at) = args.iter().position(|a| a == "--plugin-dir") {
        if let Some((_, read)) = scope.iter_mut().find(|(key, _)| key == guard::RUN_READ) {
            if !read.is_empty() { read.push('\n'); }
            read.push_str(&args[at + 1]);
        }
    }
    paths::write_atomic(&settings, &serde_json::to_vec_pretty(&guard::settings_for(&scope, &std::env::current_exe()?))?)?;
    let mut cmd = ai::prepare_claude(&p, &args, &v.root, true)?;
    cmd.envs(scope.iter().map(|(k, x)| (k.as_str(), x.as_str())));
    let _one = lock(&STARTING);
    make_room(&c.id)?;
    let mut child = cmd.spawn().map_err(|e| anyhow!("couldn't start Claude Code: {e}"))?;
    let gen = GEN.fetch_add(1, Ordering::SeqCst);
    let stopped = Arc::new(AtomicBool::new(false));
    {
        let mut st = lock(&c.state);
        st.proc = Some(Proc { gen, pid: child.id(), mode, profile: p.id.clone(), skills: skills.clone(), stdin: Arc::new(Mutex::new(child.stdin.take())), stopped: stopped.clone(), ending: false });
        // A new process numbers its blocks from b1 again.
        st.renders.clear();
        st.touch();
    }
    if !resume {
        // Listed with the vault's conversations — never made the panel's current one (D44).
        terminal::remember(&terminal::vault_store_key(&v), &c.id);
    }
    watch(c.clone(), v, gen, child, stopped, terminal::hold(p.id.clone(), Box::new(ready)));
    sweeper();
    Ok(())
}

/// Room for one more process: while `MAX_LIVE` run, the longest idle ends (D38) — never one answering (final review
/// F4): when that is all there is, the new one is refused and nothing stops.
fn make_room(except: &str) -> Result<()> {
    let mut live: Vec<(bool, Instant, Arc<Conv>)> = all()
        .into_iter()
        .filter(|c| c.id != except)
        .filter_map(|c| {
            let st = lock(&c.state);
            let key = st.running().map(|_| (st.busy, st.last_active));
            drop(st);
            key.map(|(busy, at)| (busy, at, c))
        })
        .collect();
    live.sort_by_key(|(busy, at, _)| (*busy, *at));
    let over = (live.len() + 1).saturating_sub(MAX_LIVE);
    if live.iter().take(over).any(|(busy, ..)| *busy) {
        bail!("{MAX_LIVE} conversations are answering. Wait for one, or stop one.");
    }
    for (_, _, c) in live.into_iter().take(over) {
        end(&mut lock(&c.state));
        c.changed.notify_all();
    }
    Ok(())
}

/// End `st`'s process: Echo stops it (its exit is no crash) and the conversation resumes later. False: none ran.
fn end(st: &mut State) -> bool {
    let Some(p) = st.proc.as_mut().filter(|p| !p.ending) else { return false };
    p.ending = true;
    p.stopped.store(true, Ordering::SeqCst);
    // Its input closes (a write going on may hold it: the stop below ends that too).
    if let Ok(mut s) = p.stdin.try_lock() {
        s.take();
    }
    // TERM now — Echo quitting doesn't wait for a thread (K12-12) —, KILL to what is left 3 s later, in the background.
    let pid = p.pid;
    ai::term_group(pid);
    #[cfg(test)]
    if NO_ESCALATION.load(Ordering::SeqCst) {
        return true;
    }
    std::thread::spawn(move || ai::finish_stop_group(pid));
    true
}

/// (tests) `end` leaves out its KILL thread: what it does at once has to stop a process (quitting doesn't wait).
#[cfg(test)]
static NO_ESCALATION: AtomicBool = AtomicBool::new(false);

/// A process's last lines on stderr, and whether it has ended.
#[derive(Default)]
struct Tail {
    lines: VecDeque<String>,
    done: bool,
}

/// The process's output on threads of its own: stdout's lines through the parser into the conversation's events (a
/// tick when none comes, so text before a pause shows), stderr's last lines for a crash; then its end, and the profile's
/// lease (`held`) goes with it.
fn watch(c: Arc<Conv>, v: Vault, gen: u64, mut child: Child, stopped: Arc<AtomicBool>, held: Box<dyn Send>) {
    let (tx, rx) = mpsc::channel::<String>();
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || stream::each_line(out, |l| tx.send(l).is_ok()));
    }
    let tail: Arc<(Mutex<Tail>, Condvar)> = Arc::default();
    match child.stderr.take() {
        Some(err) => {
            let tail = tail.clone();
            std::thread::spawn(move || {
                stream::each_line(err, |l| {
                    let mut t = lock(&tail.0);
                    t.lines.push_back(l);
                    if t.lines.len() > 5 {
                        t.lines.pop_front();
                    }
                    true
                });
                lock(&tail.0).done = true;
                tail.1.notify_all();
            });
        }
        None => lock(&tail.0).done = true,
    }
    std::thread::spawn(move || {
        let mut parser = stream::Parser::new(&v);
        let mut begun = false;
        loop {
            match rx.recv_timeout(TICK) {
                Ok(line) => {
                    // The lines Echo acts on itself: a control request, and Claude Code's own refusal of a call.
                    let raw = (line.contains("\"control_request\"") || line.contains("\"permission_denied\"")).then(|| serde_json::from_str::<Value>(&line).ok()).flatten().filter(|x| x["type"] == "control_request" || (x["type"] == "system" && x["subtype"] == "permission_denied"));
                    let events = parser.feed(&line);
                    // Its `init` line: Claude Code has the session now, so from here on it resumes. A first process that
                    // dies before that leaves none, and the next start is new again.
                    if !begun && parser.session_id().is_some() {
                        begun = true;
                        lock(&c.state).started = true;
                    }
                    took(&c, &v, events, raw);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let events = parser.tick(Instant::now());
                    if !events.is_empty() {
                        took(&c, &v, events, None);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        took(&c, &v, parser.finish(), None);
        let code = child.wait().ok().and_then(|s| s.code());
        let words = {
            let (m, cv) = &*tail;
            let t = cv.wait_timeout_while(lock(m), Duration::from_millis(500), |t| !t.done).unwrap_or_else(|e| e.into_inner()).0;
            t.lines.iter().rev().map(|l| l.trim().to_string()).find(|l| !l.is_empty())
        };
        exited(&c, &v, gen, stopped.load(Ordering::SeqCst), code, words);
        drop(held);
    });
}

/// The answer to control request `id`.
fn response(id: &str, r: Value) -> Value {
    json!({"type": "control_response", "response": {"subtype": "success", "request_id": id, "response": r}})
}

/// Events from the process's output, and the line itself when Echo acts on it (`raw`: a control request, or Claude
/// Code's `permission_denied`). A permission request becomes a card with Echo's diff (K12-08) that waits for the user
/// (`permit`; Stop denies it) — or Echo answers it, for what the user allowed for the rest of the conversation; any other
/// control request gets an error answer, so Claude Code never waits for Echo. A call the mode asks about, refused by
/// Claude Code itself and not by the guard, means this Claude Code doesn't hand its prompts to Echo (ruling R28). An
/// allowed edit that comes back done is a line of the vault's activity (ruling R25); the turn's end commits what it
/// edited (K12-14).
fn took(c: &Conv, v: &Vault, mut events: Vec<Value>, raw: Option<Value>) {
    if events.is_empty() && raw.is_none() {
        return; // nothing to wake the page for
    }
    let control = raw.as_ref().filter(|r| r["type"] == "control_request");
    // The card reads the file and asks git: before the lock.
    let card = control.filter(|r| r["request"]["subtype"] == "can_use_tool").map(|r| permits::card(v, &r["request"]));
    let refused = raw.as_ref().filter(|r| r["type"] == "system");
    let (mut answer, mut done, mut ended) = (None, vec![], None);
    {
        let mut st = lock(&c.state);
        if let Some(r) = control {
            let id = r["request_id"].as_str().unwrap_or("").to_string();
            let req = &r["request"];
            match card {
                Some(card) => {
                    let mut domain = None;
                    if let Some(e) = events.iter_mut().find(|e| e["type"] == "permission" && e["request"] == id.as_str()) {
                        domain = e["domain"].as_str().map(str::to_owned);
                        if let Some(o) = e.as_object_mut() {
                            o.extend(card.fields);
                        }
                    }
                    // Clean before the turn: the first time the turn asks about the file, before any edit of it is made.
                    if let (Some(e), Some(clean)) = (&card.edit, card.clean) {
                        st.this_turn.clean.entry(e.full.clone()).or_insert(clean);
                    }
                    let echos = match req["tool_name"].as_str() {
                        Some("Edit" | "Write") => st.always_edits,
                        Some("WebFetch") => domain.as_ref().is_some_and(|d| st.always_domains.contains(d)),
                        _ => false,
                    };
                    if echos {
                        if let (Some(e), Some(call)) = (card.edit, req["tool_use_id"].as_str()) {
                            st.this_turn.allowed.insert(call.to_owned(), e);
                        }
                        events.push(json!({"type": "answered", "request": id, "allow": true, "always": true}));
                        answer = st.proc.as_ref().map(|p| (p.stdin.clone(), response(&id, json!({"behavior": "allow", "updatedInput": req["input"]}))));
                    } else {
                        st.pending.insert(id, Pending { request: req.clone(), edit: card.edit, domain });
                    }
                }
                None => {
                    let error = format!("Echo doesn't answer {} requests", req["subtype"].as_str().unwrap_or("these"));
                    answer = st.proc.as_ref().map(|p| (p.stdin.clone(), json!({"type": "control_response", "response": {"subtype": "error", "request_id": id, "error": error}})));
                }
            }
        }
        if let Some(r) = refused {
            let asked = st.proc.as_ref().map(|p| p.mode.asked()).unwrap_or_default();
            let tool = r["tool_name"].as_str().unwrap_or("");
            if asked.contains(&tool) && !r["message"].as_str().unwrap_or("").contains(guard::VAULT_WHO) {
                st.this_turn.unasked = true;
            }
        }
        let mut after = vec![];
        for e in events {
            match e["type"].as_str() {
                Some("tool_result") => {
                    if let Some(x) = e["id"].as_str().and_then(|id| st.this_turn.allowed.remove(id)).filter(|_| e["ok"] == true) {
                        st.this_turn.edited.insert(x.rel.clone(), x.full.clone());
                        done.push(x);
                    }
                }
                Some("result") => {
                    st.busy = false;
                    st.started = true;
                    st.pending.clear();
                    st.touch();
                    let t = std::mem::take(&mut st.this_turn);
                    // This Claude Code doesn't hand its prompts to Echo (K12-08); the terminal panel asks there.
                    if t.unasked {
                        after.push(json!({"type": "error", "text": "This Claude Code doesn't hand permission prompts to Echo", "action": "terminal"}));
                    }
                    ended = Some(t);
                }
                _ => {}
            }
            st.push(e);
        }
        for e in after {
            st.push(e);
        }
        c.changed.notify_all();
    }
    if let Some((stdin, a)) = answer {
        let _ = write_line(&stdin, &a);
    }
    for e in &done {
        permits::record(v, &c.id, e);
    }
    if let Some(t) = ended {
        committed(c, v, &t);
    }
}

/// A turn is over: when the vault commits — its switch as it is now (X-G4) —, what it edited that was clean before it is
/// committed (K12-14), and its notices say so.
fn committed(c: &Conv, v: &Vault, t: &permits::Turn) {
    if t.edited.is_empty() {
        return;
    }
    let Ok(v) = super::get(&v.id) else { return };
    let told = lock(&c.state).told.clone();
    let (notices, reasons) = permits::commit(&v, t, &told);
    if notices.is_empty() {
        return;
    }
    let mut st = lock(&c.state);
    st.told.extend(reasons);
    for n in notices {
        st.push(n);
    }
    c.changed.notify_all();
}

/// The process ended: a turn still going ends — stopped by Echo, or with an error saying why —, and what it edited is
/// committed as at a turn's end.
fn exited(c: &Conv, v: &Vault, gen: u64, stopped: bool, code: Option<i32>, words: Option<String>) {
    let t = {
        let mut st = lock(&c.state);
        let current = st.proc.as_ref().is_some_and(|p| p.gen == gen);
        if current {
            st.proc = None;
        }
        // Cards nobody can answer now.
        for id in std::mem::take(&mut st.pending).into_keys() {
            st.push(json!({"type": "answered", "request": id, "allow": false, "always": false}));
        }
        if st.busy {
            st.busy = false;
            if stopped {
                st.push(json!({"type": "notice", "kind": "stopped", "text": "Stopped"}));
            } else {
                let why = match (words, code) {
                    (Some(w), _) => format!(": {}", w.chars().take(300).collect::<String>()),
                    (None, Some(n)) => format!(" (exit code {n})"),
                    (None, None) => String::new(),
                };
                st.push(json!({"type": "error", "text": format!("Claude Code stopped before the answer ended{why}")}));
            }
        }
        st.touch();
        c.changed.notify_all();
        if current { std::mem::take(&mut st.this_turn) } else { permits::Turn::default() }
    };
    committed(c, v, &t);
}

/// A conversation Echo holds no events of shows what Claude Code's transcript has (K12-11), numbered as live events
/// are, so a message sent later follows them without renumbering; a transcript that can't be read is said in a notice.
/// One that doesn't hold its beginning any more (its events let go, `sweep`; maybe a message since) is read again from
/// its transcript, whole, when a page asks `from` before what it holds — one opening it (final review X2) —: in place
/// of what it holds, numbered after every number it gave, so a page that kept polling is answered from elsewhere than
/// it asked and takes the conversation whole again (`knChatTake`), never twice. A page still showing it (its `from`
/// inside), or a message (`from` None), goes on as it is. While a turn goes it waits for the turn's end (`due`). Read,
/// it counts as used: the sweep leaves it for the page that opened it.
fn open_history(c: &Conv, v: &Vault, from: Option<u64>) {
    {
        let mut st = lock(&c.state);
        if !st.started || st.history || !(st.end() == 0 || st.due || from.is_some_and(|f| f < st.start)) {
            return;
        }
        if st.busy || !st.pending.is_empty() {
            st.due = true;
            return;
        }
    }
    let past = transcript::events(&terminal::claude_folder_of(&c.profile), &c.id, v);
    let mut st = lock(&c.state);
    // Read by another meanwhile, or a turn began.
    if st.history || st.busy || !st.pending.is_empty() {
        return;
    }
    st.history = true;
    st.due = false;
    st.last_active = now();
    match past {
        Some(events) => {
            if st.end() > 0 {
                st.start = st.end() + 1;
            }
            st.events = VecDeque::new();
            st.renders.clear();
            events.into_iter().for_each(|e| st.push(e));
        }
        None if st.events.is_empty() => st.push(json!({"type": "notice", "kind": "history", "text": "Its history can't be shown here; continuing works."})),
        None => {}
    }
    c.changed.notify_all();
}

/// The events of conversation `conversation` of vault `vault` from event `from` on; waits up to `wait` for some (or
/// for its state to change) when there are none yet (K12-06). None: no such conversation of the vault.
pub fn read(vault: &str, conversation: &str, from: u64, wait: Duration) -> Option<ChatChunk> {
    let v = super::get(vault).ok()?;
    let c = conv_of(&v, conversation)?;
    open_history(&c, &v, Some(from));
    let deadline = Instant::now() + wait;
    let mut st = lock(&c.state);
    // A page opening it uses it (final review X2).
    if from == 0 || from < st.start {
        st.last_active = now();
    }
    let seen = st.flags();
    while st.end() == from && st.flags() == seen {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        st = c.changed.wait_timeout(st, left).unwrap_or_else(|e| e.into_inner()).0;
    }
    // Asked for events it no longer holds, or from an earlier run of Echo: from its first.
    let at = if from < st.start || from > st.end() { st.start } else { from };
    let events: Vec<Value> = st.events.iter().skip((at - st.start) as usize).take(CHUNK).cloned().collect();
    Some(ChatChunk { next: at + events.len() as u64, events, live: st.proc.is_some(), busy: st.busy, waiting: !st.pending.is_empty() })
}

/// Stop the conversation's turn (K12-09): its waiting permission requests are denied, the turn interrupted, and its
/// process ended if the turn hasn't ended `STOP_AFTER` later (the conversation stays resumable).
pub fn stop(conversation: &str) {
    let Some(c) = lock(&CHATS).get(conversation).cloned() else { return };
    let (stdin, gen, turn, denied) = {
        let mut st = lock(&c.state);
        let Some((stdin, gen)) = st.running().map(|p| (p.stdin.clone(), p.gen)) else { return };
        if !st.busy {
            return;
        }
        let denied: Vec<String> = std::mem::take(&mut st.pending).into_keys().collect();
        for id in &denied {
            st.push(json!({"type": "answered", "request": id, "allow": false, "always": false}));
        }
        c.changed.notify_all();
        (stdin, gen, st.turn, denied)
    };
    // The deadline first: the writes below wait while Claude Code doesn't read its input.
    let deadline = now() + STOP_AFTER;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(50));
        let mut st = lock(&c.state);
        if !st.busy || st.turn != turn || st.proc.as_ref().is_none_or(|p| p.gen != gen) {
            return;
        }
        if now() >= deadline {
            end(&mut st);
            c.changed.notify_all();
            return;
        }
    });
    for id in denied {
        let _ = write_line(&stdin, &json!({"type": "control_response", "response": {"subtype": "success", "request_id": id, "response": {"behavior": "deny", "message": "The user stopped this turn."}}}));
    }
    let _ = write_line(&stdin, &json!({"type": "control_request", "request_id": format!("echo-stop-{gen}-{turn}"), "request": {"subtype": "interrupt"}}));
}

/// Answer permission request `request` of conversation `conversation` (`kn_chat_permit`, K12-08): allow it once, or
/// with `always` for the rest of the conversation — every later edit, or that page's domain, which Echo then answers
/// itself (the guard still applies to each) —, or deny it, `message` going to Claude with the refusal.
pub fn permit(conversation: &str, request: &str, allow: bool, always: bool, message: Option<String>) -> Result<()> {
    let c = lock(&CHATS).get(conversation).cloned().ok_or_else(|| anyhow!("no such conversation"))?;
    let (stdin, answer) = {
        let mut st = lock(&c.state);
        let stdin = st.running().map(|p| p.stdin.clone()).ok_or_else(|| anyhow!("its Claude Code isn't running any more"))?;
        let p = st.pending.remove(request).ok_or_else(|| anyhow!("that question isn't waiting any more"))?;
        let r = if allow {
            match p.request["tool_name"].as_str() {
                Some("Edit" | "Write") if always => st.always_edits = true,
                Some("WebFetch") if always => st.always_domains.extend(p.domain),
                _ => {}
            }
            if let (Some(e), Some(call)) = (p.edit, p.request["tool_use_id"].as_str()) {
                st.this_turn.allowed.insert(call.to_owned(), e);
            }
            json!({"behavior": "allow", "updatedInput": p.request["input"]})
        } else {
            let said: String = message.as_deref().map(str::trim).unwrap_or("").chars().take(TEXT_MAX).collect();
            let m = if said.is_empty() { "The user declined this.".to_string() } else { format!("The user declined this and said: {said}") };
            json!({"behavior": "deny", "message": m})
        };
        st.push(json!({"type": "answered", "request": request, "allow": allow, "always": allow && always}));
        c.changed.notify_all();
        (stdin, response(request, r))
    };
    write_line(&stdin, &answer).map_err(|e| anyhow!("Claude Code didn't take the answer: {e}"))
}

/// End the processes of the conversations on `profile` (a sign-in needs the profile to itself); how many. They resume.
pub fn close_profile(profile: &str) -> usize {
    let mut n = 0;
    for c in all() {
        let mut st = lock(&c.state);
        if st.running().is_some_and(|p| p.profile == profile) && end(&mut st) {
            n += 1;
            c.changed.notify_all();
        }
    }
    n
}

/// Echo is going: every conversation's process ends.
pub fn close_all() {
    for c in all() {
        if end(&mut lock(&c.state)) {
            c.changed.notify_all();
        }
    }
}

/// The conversations that have a process (also one still stopping).
pub fn live() -> Vec<String> {
    all().into_iter().filter(|c| lock(&c.state).proc.is_some()).map(|c| c.id.clone()).collect()
}

/// End the processes idle for `IDLE` with no turn going (they resume with the next message); and let go of the events
/// of conversations with no process, idle as long (final review F5): their transcript has them, read again when the
/// conversation is opened again (`open_history`), numbered on from where they were.
pub fn sweep() {
    let t = now();
    for c in all() {
        // One being sent to is in use.
        let Ok(_one) = c.sending.try_lock() else { continue };
        let mut st = lock(&c.state);
        let idle = !st.busy && t.saturating_duration_since(st.last_active) >= IDLE;
        if idle && st.running().is_some() && end(&mut st) {
            c.changed.notify_all();
        } else if idle && st.proc.is_none() && st.started && !st.events.is_empty() {
            st.start = st.end();
            st.events = VecDeque::new();
            st.renders.clear();
            st.history = false;
        }
    }
}

/// `sweep`, every 30 s from the first start on.
static SWEEPER: std::sync::Once = std::sync::Once::new();

fn sweeper() {
    // Tests sweep by themselves: a sweep of its own could fire between a test's clock skew and its check.
    #[cfg(not(test))]
    SWEEPER.call_once(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(30));
            sweep();
        });
    });
}

/// Was conversation `id` of `v` ever run by Ask (Echo's folder of it, `chat/<id>/`, is there): continued in the terminal
/// panel it keeps its prompts (D41) — it may have read web pages or others' words.
pub fn started_in_ask(v: &Vault, id: &str) -> bool {
    valid_id(id) && super::cache_dir(&v.id).join("chat").join(id).is_dir()
}

/// The vault's conversations for Ask's list (the `chats` route): those of its list Claude Code kept and those Echo
/// holds, newest first — `{id, at, title, live, busy}`; a title is the first thing the user asked, read from Claude Code's
/// transcript (`terminal::conversations`), else from the conversation's own first message.
pub fn list_value(v: &Vault) -> Value {
    let mut out: Vec<Value> = terminal::conversations(&terminal::vault_store_key(v)).as_array().cloned().unwrap_or_default();
    for c in all().into_iter().filter(|c| c.vault == v.id && c.profile == v.profile) {
        let st = lock(&c.state);
        let asked = st.events.iter().find(|e| e["type"] == "user").and_then(|e| e["text"].as_str()).map(|t| t.chars().take(90).collect::<String>());
        let i = match out.iter().position(|x| x["id"] == c.id.as_str()) {
            Some(i) => i,
            None => {
                out.push(json!({"id": c.id, "at": null, "title": null}));
                out.len() - 1
            }
        };
        let row = &mut out[i];
        if st.active_at.is_some() {
            row["at"] = json!(st.active_at);
        }
        if row["title"].is_null() {
            row["title"] = json!(asked);
        }
        row["live"] = json!(st.proc.is_some());
        row["busy"] = json!(st.busy);
    }
    for row in out.iter_mut().filter(|r| r.get("live").is_none()) {
        row["live"] = json!(false);
        row["busy"] = json!(false);
    }
    out.sort_by(|a, b| b["at"].as_str().unwrap_or("").cmp(a["at"].as_str().unwrap_or("")));
    Value::Array(out)
}

/// Take a conversation to the terminal panel (`kn_chat_terminal`): its process here ends first (one process per
/// conversation, K12-12), then the line that resumes it there (None on a managed profile: the panel resumes it by id).
pub fn to_terminal(vault: &str, conversation: &str) -> Result<Value> {
    let v = super::get(vault)?;
    let c = conv_of(&v, conversation).ok_or_else(|| anyhow!("that conversation isn't one of this vault's"))?;
    let mut st = lock(&c.state);
    end(&mut st);
    let (st, _) = c.changed.wait_timeout_while(st, Duration::from_secs(10), |st| st.proc.is_some()).unwrap_or_else(|e| e.into_inner());
    if st.proc.is_some() {
        bail!("its Claude Code is still stopping — try again in a moment");
    }
    drop(st);
    Ok(json!({"conversation": c.id, "command": terminal::resume_command(&v.id, &c.id)}))
}

/// (tests) No conversation from a test before: every process stopped, the clock back.
#[cfg(test)]
pub(crate) fn reset() {
    close_all();
    let t = Instant::now();
    while all().iter().any(|c| lock(&c.state).proc.is_some()) && t.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    lock(&CHATS).clear();
    *lock(&SKEW) = Duration::ZERO;
    NO_ESCALATION.store(false, Ordering::SeqCst);
}

/// (tests) The clock runs `by` further ahead (idle stops, a stop's 5 s).
#[cfg(test)]
pub(crate) fn skew(by: Duration) {
    *lock(&SKEW) += by;
}

/// (tests) The conversation's events until its turn has ended (its `result`, `error` or `notice`, not busy).
#[cfg(test)]
pub(crate) fn turn_events(vault: &str, id: &str) -> Vec<Value> {
    events_until(vault, id, |ev, c| !c.busy && ev.iter().any(|e| matches!(e["type"].as_str(), Some("result" | "error" | "notice"))))
}

/// (tests) The conversation's events from the start until `done` says so (10 s at most).
#[cfg(test)]
pub(crate) fn events_until(vault: &str, id: &str, done: impl Fn(&[Value], &ChatChunk) -> bool) -> Vec<Value> {
    let t = std::time::Instant::now();
    let (mut all, mut from) = (vec![], 0);
    loop {
        let c = read(vault, id, from, Duration::from_millis(200)).expect("the conversation");
        from = c.next;
        all.extend(c.events.iter().cloned());
        if done(&all, &c) {
            return all;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "timed out: {all:#?}");
    }
}

/// (tests) A turn as Claude Code streams it: one text block, then the `result`.
#[cfg(test)]
pub(crate) fn turn(text: &str) -> String {
    [
        json!({"type": "system", "subtype": "init", "session_id": "made-up", "tools": ["Read"]}),
        json!({"type": "stream_event", "event": {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}, "parent_tool_use_id": null}),
        json!({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": text}}, "parent_tool_use_id": null}),
        json!({"type": "stream_event", "event": {"type": "content_block_stop", "index": 0}, "parent_tool_use_id": null}),
        json!({"type": "result", "subtype": "success", "is_error": false, "stop_reason": "end_turn", "duration_ms": 900, "usage": {"input_tokens": 12, "output_tokens": 7}}),
    ]
    .iter()
    .map(|l| format!("{l}\n"))
    .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn args(v: &Vault, p: &ai::Profile, mode: Mode, session: &Session, settings: &Path) -> Result<Vec<String>> {
        super::args(v, p, mode, session, settings, &skills::Catalog::load(v, p)?)
    }

    fn scope(v: &Vault, p: &ai::Profile, mode: Mode) -> Vec<(String, String)> {
        super::scope(v, mode, &skills::Catalog::load(v, p).unwrap())
    }
    use crate::ai::fake;
    use crate::knowledge::{fixture, AddOpts};
    use std::path::PathBuf;
    use std::time::Instant;

    const ID: &str = "0f6ac8d0-1111-4222-8333-444455556666";
    const ASK: &str = "Read,Grep,Glob,Skill,Agent";

    /// Stops this test's conversations when it ends (declared after the fake and the folders: dropped first).
    pub(super) struct Clean;

    impl Drop for Clean {
        fn drop(&mut self) {
            reset();
        }
    }

    pub(super) fn clean() -> Clean {
        reset();
        Clean
    }

    pub(super) fn vault(root: &Path, profile: &str) -> Vault {
        let t = fixture::twin(&root.join("twin"));
        crate::knowledge::add(&t, AddOpts { name: Some("Twin".into()), profile: Some(profile.into()), ..Default::default() }).unwrap()
    }

    fn profile(id: &str, kind: &str) -> ai::Profile {
        serde_json::from_value(json!({"id": id, "name": id, "kind": kind})).unwrap()
    }

    /// A managed profile signed in, answered by the fake.
    pub(super) fn signed_in() -> ai::Profile {
        fake::profile("w", None, &fake::signed_in("pat@example.com"))
    }

    fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
    }

    pub(super) fn file(p: &ai::Profile, name: &str) -> String {
        std::fs::read_to_string(fake::file(p, name)).unwrap_or_default()
    }

    fn argv(p: &ai::Profile) -> Vec<String> {
        file(p, "argv").lines().map(str::to_owned).collect()
    }

    pub(super) fn script(p: &ai::Profile, name: &str, lines: &str) {
        std::fs::write(fake::file(p, name), lines).unwrap();
    }

    pub(super) fn of_type<'a>(ev: &'a [Value], t: &str) -> Vec<&'a Value> {
        ev.iter().filter(|e| e["type"] == t).collect()
    }

    fn gone(pid: &str) -> bool {
        unsafe { libc::kill(pid.trim().parse().unwrap(), 0) != 0 }
    }

    pub(super) fn until(what: &str, f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    #[test]
    fn conversation_args_per_profile() {
        let (_env, root) = crate::paths::testenv::folders("chat-args");
        let v = vault(&root, "system");
        // The user's "no permission prompts" is for their own folder session, never a conversation (D24).
        let mut cfg = crate::config::Config::load().unwrap();
        cfg.folder_skip_permissions = true;
        cfg.save().unwrap();
        let settings = root.join("settings.json");
        let first = "What did Northwind Retail decide about pricing and the rollout plan?";
        let system = profile("system", "system");
        let a = args(&v, &system, Mode::Ask, &Session::New { id: ID.into(), first: first.into() }, &settings).unwrap();
        for (flag, want) in [
            ("--input-format", "stream-json"),
            ("--output-format", "stream-json"),
            ("--permission-mode", "default"),
            ("--permission-prompts", "host"),
            ("--permission-prompt-tool", "stdio"),
            ("--tools", ASK),
            ("--allowed-tools", ASK),
            ("--settings", settings.to_str().unwrap()),
            ("--session-id", ID),
            ("--name", "Twin: What did Northwind Retail decide about p"),
        ] {
            assert_eq!(value_of(&a, flag), Some(want), "{flag} in {a:?}");
        }
        for flag in ["-p", "--verbose", "--include-partial-messages", "--prompt-suggestions", "--strict-mcp-config"] {
            assert!(a.iter().any(|x| x == flag), "{flag} in {a:?}");
        }
        assert!(!a.iter().any(|x| x == "--dangerously-skip-permissions" || x == "--resume"), "{a:?}");
        assert!(!a.iter().any(|x| x == "--plugin-dir" || x == "--setting-sources"), "as configured, Claude Code loads the vault itself: {a:?}");
        // A managed profile: the vault's skills as a plugin (its own settings only come from `ai::build_command`).
        let managed = profile("work", "claude");
        let c = args(&v, &managed, Mode::Curate, &Session::Resume(ID.into()), &settings).unwrap();
        assert_eq!(value_of(&c, "--resume"), Some(ID));
        assert!(!c.iter().any(|x| x == "--session-id" || x == "--name" || x == "--dangerously-skip-permissions"), "{c:?}");
        assert!(value_of(&c, "--plugin-dir").is_some_and(|d| Path::new(d).join(".claude-plugin/plugin.json").is_file()), "{c:?}");
        assert_eq!(value_of(&c, "--tools"), Some("Read,Grep,Glob,Skill,Agent,Edit,Write"));
        assert_eq!(value_of(&c, "--allowed-tools"), Some(ASK), "Curate's edits are asked about");
        let r = args(&v, &managed, Mode::Research, &Session::Resume(ID.into()), &settings).unwrap();
        assert_eq!(value_of(&r, "--tools"), Some("Read,Grep,Glob,Skill,Agent,WebSearch,WebFetch"));
        assert_eq!(value_of(&r, "--allowed-tools"), Some("Read,Grep,Glob,Skill,Agent,WebSearch"), "each fetched page is asked about");
        for x in [&a, &c, &r] {
            let tools = value_of(x, "--tools").unwrap();
            assert!(!tools.split(',').any(|t| t == "Bash" || t == "NotebookEdit" || t.starts_with("mcp__")), "{tools}");
        }
    }

    #[test]
    fn settings_env_carries_the_scope() {
        let (_env, root) = crate::paths::testenv::folders("chat-scope");
        let v = vault(&root, "system");
        let env = |p: &ai::Profile, mode: Mode, v: &Vault| guard::settings_for(&scope(v, p, mode), Path::new("/opt/echo/echoctl"))["env"].clone();
        let e = env(&profile("work", "claude"), Mode::Curate, &v);
        assert_eq!(e[guard::RUN_MODE], "vault");
        assert_eq!(e[guard::RUN_VAULT], v.root.to_str().unwrap());
        assert_eq!(e[guard::RUN_CHAT], "curate");
        assert_eq!(e[guard::RUN_SOURCES], "raw_inbox");
        assert_eq!(e[guard::RUN_SKILLS], "twin:c\ntwin:s", "a managed profile's skills come from the vault's plugin");
        assert_eq!(env(&profile("system", "system"), Mode::Ask, &v)[guard::RUN_SKILLS], "c\ns", "as configured, by their own names");
        // The vault's own sources folder, or raw_inbox when it has none (ruling R11).
        let other = root.join("notes");
        fixture::write(&other, "inbox/call.md", "# A call\n");
        fixture::write(&other, "a.md", "# A\n");
        let o = crate::knowledge::add(&other, AddOpts { name: Some("Notes".into()), ..Default::default() }).unwrap();
        assert_eq!(env(&profile("system", "system"), Mode::Research, &o)[guard::RUN_SOURCES], "inbox");
        let bare = root.join("bare");
        fixture::write(&bare, "a.md", "# A\n");
        let b = crate::knowledge::add(&bare, AddOpts { name: Some("Bare".into()), ..Default::default() }).unwrap();
        let e = env(&profile("system", "system"), Mode::Ask, &b);
        assert_eq!(e[guard::RUN_SOURCES], "raw_inbox");
        assert_eq!(e[guard::RUN_SKILLS], "");
        assert_eq!(e[guard::RUN_CHAT], "ask");
    }

    #[test]
    fn conversations_explain_which_skills_the_guard_allows() {
        let (_env, root) = crate::paths::testenv::folders("chat-skill-prompt");
        let v = vault(&root, "system");
        for p in [profile("system", "system"), profile("work", "claude")] {
            for mode in [Mode::Ask, Mode::Curate, Mode::Research] {
                let a = args(&v, &p, mode, &Session::Resume(ID.into()), &root.join("settings.json")).unwrap();
                assert_eq!(a.iter().filter(|x| *x == "--append-system-prompt").count(), 1);
                let prompt = value_of(&a, "--append-system-prompt").expect("the conversation's skill limits");
                let names = serde_json::to_string(&skills::Catalog::load(&v, &p).unwrap().names()).unwrap();
                assert!(prompt.contains(&names), "{prompt}");
                assert!(prompt.contains("User-level skills and commands are supported directly"), "{prompt}");
                assert!(prompt.contains("Read, Grep and Glob"), "a question needs no skill: {prompt}");
                if p.kind.managed() {
                    assert!(prompt.contains("AGENTS.md"), "keep the managed profile's vault rules: {prompt}");
                }
            }
        }
        std::fs::remove_file(v.root.join(".claude/skills/s/SKILL.md")).unwrap();
        std::fs::remove_file(v.root.join(".claude/commands/c.md")).unwrap();
        let a = args(&v, &profile("system", "system"), Mode::Ask, &Session::Resume(ID.into()), &root.join("settings.json")).unwrap();
        assert!(value_of(&a, "--append-system-prompt").unwrap().contains("user skills and this vault: []"));
    }

    #[test]
    fn a_new_process_uses_current_vault_skills_for_its_prompt_plugin_and_guard() {
        let (_env, root) = crate::paths::testenv::folders("chat-current-skills");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        // The registry still has s and c: change the vault without asking Echo to detect it again.
        std::fs::remove_file(v.root.join(".claude/skills/s/SKILL.md")).unwrap();
        fixture::write(&v.root, ".claude/skills/twin/SKILL.md", "---\nname: twin\ndescription: Answer from this vault.\n---\nRead knowledge/CATALOG.md.\n");
        script(&p, "chat.jsonl", &turn("From the vault."));
        let id = send(&v.id, None, "What do I know about the flywheel?", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        let a = argv(&p);
        let settings: Value = serde_json::from_slice(&std::fs::read(value_of(&a, "--settings").unwrap()).unwrap()).unwrap();
        let env = settings["env"].as_object().unwrap();
        assert_eq!(env[guard::RUN_SKILLS], "twin:c\ntwin:twin");
        let prompt = value_of(&a, "--append-system-prompt").unwrap();
        assert!(prompt.contains(r#"["twin:c","twin:twin"]"#), "{prompt}");
        let plugin = Path::new(value_of(&a, "--plugin-dir").unwrap());
        assert!(plugin.join("skills/twin/SKILL.md").is_file());
        assert!(!plugin.join("skills/s/SKILL.md").exists());
        let guard = guard::Scope::from_vars(|k| env.get(k).and_then(Value::as_str).map(Into::into)).unwrap();
        let skill = |s: &str| json!({"tool_name": "Skill", "tool_input": {"skill": s}});
        assert!(guard.judge(&skill("twin:twin")).is_ok());
        for s in ["twin:s", "personal-twin", "twin"] {
            assert!(guard.judge(&skill(s)).is_err(), "{s}");
        }
        assert!(guard.judge(&json!({"tool_name": "Read", "tool_input": {"file_path": v.root.join("knowledge/CATALOG.md")}})).is_ok());
        assert!(guard.judge(&json!({"tool_name": "Bash", "tool_input": {"command": "pwd"}})).is_err());
    }

    #[test]
    fn personal_skills_work_without_copying_and_keep_the_conversations_boundaries() {
        let (_env, root) = crate::paths::testenv::folders("chat-personal-skill");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let user = p.config_folder();
        fixture::write(&user, "commands/personal-twin.md", "Read the vault's catalog to answer $ARGUMENTS.");
        fixture::write(&user, "commands/other.md", "Another installed command.");
        fixture::write(&user, "skills/review/SKILL.md", "---\nname: review\n---\nRead references.md.");
        fixture::write(&user, "skills/review/references.md", "Use evidence from the vault.");
        fixture::write(&user, "credentials.json", "made-up token");
        fixture::write(&v.root, ".claude/commands/tools/review.md", "---\nname: ignored\n---\nReview a page.");
        let before = fixture::snapshot(&v.root);
        script(&p, "chat.jsonl", &turn("The flywheel is described in the catalog."));
        let id = send(&v.id, None, "/personal-twin explain the flywheel", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        let a = argv(&p);
        let settings: Value = serde_json::from_slice(&std::fs::read(value_of(&a, "--settings").unwrap()).unwrap()).unwrap();
        let env = settings["env"].as_object().unwrap();
        let plugin = Path::new(value_of(&a, "--plugin-dir").unwrap());
        assert!(plugin.join("commands/tools/review.md").is_file(), "nested command names match the generated plugin");
        let g = guard::Scope::from_vars(|k| env.get(k).and_then(Value::as_str).map(Into::into)).unwrap();
        let tool = |name: &str, input: Value| json!({"tool_name":name,"tool_input":input});
        assert!(g.judge(&tool("Skill", json!({"skill":"personal-twin"}))).is_ok());
        assert!(g.judge(&tool("Skill", json!({"skill":"twin:tools:review"}))).is_ok());
        for f in ["commands/personal-twin.md", "skills/review/references.md"] {
            assert!(g.judge(&tool("Read", json!({"file_path":user.join(f)}))).is_ok(), "{f}");
        }
        for f in [user.join("credentials.json"), root.join("outside.md")] {
            assert!(g.judge(&tool("Read", json!({"file_path":f}))).is_err(), "{}", f.display());
        }
        assert!(g.judge(&tool("Grep", json!({"path":user,"pattern":"token"}))).is_err(), "the config folder isn't a skill");
        assert!(g.judge(&tool("Write", json!({"file_path":user.join("skills/review/new.md"),"content":"x"}))).is_err());
        assert!(g.judge(&tool("Bash", json!({"command":"pwd"}))).is_err());
        assert!(g.judge(&tool("Write", json!({"file_path":v.root.join("knowledge/new.md"),"content":"x"}))).is_err(), "Ask stays read only");
        let sent: Value = serde_json::from_str(file(&p, "chat.in.jsonl").lines().next().unwrap()).unwrap();
        assert_eq!(sent["message"]["content"][0]["text"], "/personal-twin explain the flywheel");
        assert_eq!(fixture::snapshot(&v.root), before, "no skill was copied into the vault");
        assert!(send(&v.id, Some(&id), "/unknown hello", Mode::Ask, &[]).is_err());
        // A newly installed user command becomes usable in this conversation: restart with the same session id.
        fixture::write(&user, "commands/new-skill.md", "Answer from the vault.");
        assert_eq!(send(&v.id, Some(&id), "/new-skill hello", Mode::Ask, &[]).unwrap(), id);
        events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
        assert_eq!(file(&p, "pids").lines().count(), 2);
        assert_eq!(value_of(&argv(&p), "--resume"), Some(id.as_str()));
    }

    #[test]
    fn a_conversation_streams_and_ends_its_turn() {
        let (_env, root) = crate::paths::testenv::folders("chat-turn");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let mut cfg = crate::config::Config::load().unwrap();
        cfg.folder_skip_permissions = true;
        cfg.save().unwrap();
        script(&p, "chat.1.jsonl", &turn("Northwind Retail fixes forward."));
        let id = send(&v.id, None, "What did we decide with Northwind Retail?", Mode::Ask, &[Context::Page("knowledge/clients/acme.md".into())]).unwrap();
        let ev = turn_events(&v.id, &id);
        assert_eq!(ev[0]["type"], "user");
        assert_eq!(ev[0]["text"], "What did we decide with Northwind Retail?");
        assert_eq!(ev[0]["context"], json!([{"page": "knowledge/clients/acme.md"}]));
        let texts = of_type(&ev, "text");
        assert!(texts.last().is_some_and(|t| t["done"] == true && t["html"].as_str().unwrap().contains("Northwind Retail fixes forward.")), "{ev:#?}");
        assert_eq!(of_type(&ev, "result").len(), 1);
        assert!(ev.iter().enumerate().all(|(i, e)| e["n"] == i as u64), "numbered in order: {ev:#?}");
        // §5.4's command, in the vault, on the managed profile's own settings, with the vault's skills.
        let a = argv(&p);
        assert_eq!(value_of(&a, "--session-id"), Some(id.as_str()));
        assert_eq!(value_of(&a, "--setting-sources"), Some("user"));
        assert!(value_of(&a, "--plugin-dir").is_some());
        assert!(!a.iter().any(|x| x == "--dangerously-skip-permissions"), "never, whatever the folder session's switch says");
        assert_eq!(file(&p, "cwd").trim(), v.root.to_str().unwrap());
        // The scope in the process's environment and in its own settings file (D40).
        let env = file(&p, "env");
        for want in ["ECHOCTL_RUN_MODE=vault".to_string(), "ECHOCTL_RUN_CHAT=ask".into(), format!("ECHOCTL_RUN_VAULT={}", v.root.display()), "ECHOCTL_RUN_SOURCES=raw_inbox".into(), "ECHOCTL_RUN_SKILLS=twin:c".into()] {
            assert!(env.lines().any(|l| l == want), "{want} in {env}");
        }
        let settings: PathBuf = value_of(&a, "--settings").unwrap().into();
        assert_eq!(settings, crate::knowledge::cache_dir(&v.id).join("chat").join(&id).join("settings.json"));
        let s: Value = serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
        assert_eq!(s["env"][guard::RUN_MODE], "vault");
        assert_eq!(s["env"][guard::RUN_CHAT], "ask");
        assert!(s["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().ends_with("run-guard"));
        // What Claude Code read: one stream-json user message naming the page.
        let sent: Value = serde_json::from_str(file(&p, "chat.in.jsonl").lines().next().unwrap()).unwrap();
        assert_eq!(sent["type"], "user");
        let text = sent["message"]["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("What did we decide with Northwind Retail?") && text.contains("knowledge/clients/acme.md"), "{text}");
        // Listed under the vault's key; live; the next message goes to the same process.
        assert!(crate::terminal::listed(&crate::terminal::vault_store_key(&v), &id));
        assert_eq!(live(), vec![id.clone()]);
        script(&p, "chat.2.jsonl", &turn("And the pricing stays per seat."));
        assert_eq!(send(&v.id, Some(&id), "And pricing?", Mode::Ask, &[]).unwrap(), id);
        let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
        assert_eq!(of_type(&ev, "user").len(), 2);
        assert_eq!(file(&p, "pids").lines().count(), 1, "one process for the conversation");
        // Others are refused: an empty or oversized message, a page outside the vault, a conversation not the vault's.
        assert!(send(&v.id, Some(&id), "  ", Mode::Ask, &[]).is_err());
        assert!(send(&v.id, Some(&id), &"x".repeat(20_001), Mode::Ask, &[]).is_err());
        assert!(send(&v.id, Some(&id), "Hi", Mode::Ask, &[Context::Page("../outside.md".into())]).is_err());
        assert!(send(&v.id, Some("00000000-0000-4000-8000-000000000000"), "Hi", Mode::Ask, &[]).is_err());
        assert!(read(&v.id, "../../etc", 0, Duration::ZERO).is_none());
    }

    #[test]
    fn a_message_during_a_turn_is_refused() {
        let (_env, root) = crate::paths::testenv::folders("chat-busy");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.1.jsonl", "{\"_hang\":true}\n");
        let id = send(&v.id, None, "Summarise Northwind Retail", Mode::Ask, &[]).unwrap();
        let e = send(&v.id, Some(&id), "And Acme?", Mode::Ask, &[]).unwrap_err().to_string();
        assert!(e.contains("Wait for this answer, or stop it"), "{e}");
    }

    #[test]
    fn mode_change_restarts_with_resume() {
        let (_env, root) = crate::paths::testenv::folders("chat-mode");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.jsonl", &turn("Done."));
        let id = send(&v.id, None, "What is on the Acme page?", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        let first = file(&p, "pids");
        assert_eq!(send(&v.id, Some(&id), "Add a line about the pilot", Mode::Curate, &[]).unwrap(), id);
        let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
        assert_eq!(of_type(&ev, "user").len(), 2, "one conversation: {ev:#?}");
        let a = argv(&p);
        assert_eq!(value_of(&a, "--resume"), Some(id.as_str()), "{a:?}");
        assert_eq!(value_of(&a, "--tools"), Some("Read,Grep,Glob,Skill,Agent,Edit,Write"));
        assert!(file(&p, "env").lines().any(|l| l == "ECHOCTL_RUN_CHAT=curate"));
        let settings = crate::knowledge::cache_dir(&v.id).join("chat").join(&id).join("settings.json");
        assert_eq!(serde_json::from_slice::<Value>(&std::fs::read(settings).unwrap()).unwrap()["env"][guard::RUN_CHAT], "curate");
        let pids: Vec<String> = file(&p, "pids").lines().map(str::to_owned).collect();
        assert_eq!(pids.len(), 2);
        until("the first process ended", || gone(&first));
        assert_eq!(live(), vec![id.clone()]);
        // Research only when the vault allows it (ruling R3).
        crate::knowledge::set(&v.id, "ask.research", "off").unwrap();
        let e = send(&v.id, Some(&id), "Look it up on the web", Mode::Research, &[]).unwrap_err().to_string();
        assert!(e.contains("Research"), "{e}");
    }

    #[test]
    fn stop_interrupts_then_ends() {
        let (_env, root) = crate::paths::testenv::folders("chat-stop");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let asks = json!({"type": "control_request", "request_id": "req-7", "request": {"subtype": "can_use_tool", "tool_name": "Edit", "input": {"file_path": format!("{}/knowledge/clients/acme.md", v.root.display()), "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nPilot on 18 November.\n"}}});
        let init = turn("").lines().next().unwrap().to_string(); // Claude Code's `init` comes first
        script(&p, "chat.1.jsonl", &format!("{init}\n{asks}\n{{\"_hang\":true}}\n"));
        let id = send(&v.id, None, "Note the pilot date on Acme", Mode::Curate, &[]).unwrap();
        let ev = events_until(&v.id, &id, |_, c| c.waiting);
        assert_eq!(of_type(&ev, "permission")[0]["request"], "req-7");
        stop(&id);
        // The waiting card is denied, then the turn interrupted; the fake ignores the interrupt.
        until("the deny and the interrupt", || file(&p, "chat.in.jsonl").contains("\"interrupt\""));
        let sent: Vec<Value> = file(&p, "chat.in.jsonl").lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        let deny = sent.iter().find(|l| l["type"] == "control_response").expect("an answer to the request");
        assert_eq!(deny["response"]["request_id"], "req-7");
        assert_eq!(deny["response"]["response"]["behavior"], "deny");
        assert_eq!(sent.last().unwrap()["request"]["subtype"], "interrupt");
        let ev = events_until(&v.id, &id, |ev, c| !c.waiting && !of_type(ev, "answered").is_empty());
        assert_eq!(of_type(&ev, "answered")[0]["allow"], false);
        std::thread::sleep(Duration::from_millis(300));
        let c = read(&v.id, &id, 0, Duration::ZERO).unwrap();
        assert!(c.live && c.busy, "still going before its 5 s");
        // 5 s later the process is ended, and the conversation stays resumable.
        skew(Duration::from_secs(5));
        let t = Instant::now();
        let ev = events_until(&v.id, &id, |_, c| !c.busy && !c.live);
        assert!(t.elapsed() < Duration::from_secs(3), "ended by the stop's 5 s, not by waiting: {:?}", t.elapsed());
        assert_eq!(ev.last().unwrap()["type"], "notice");
        assert_eq!(ev.last().unwrap()["kind"], "stopped");
        until("its process ended", || gone(&file(&p, "pids")));
        script(&p, "chat.2.jsonl", &turn("The pilot is noted."));
        send(&v.id, Some(&id), "Try again", Mode::Curate, &[]).unwrap();
        turn_events(&v.id, &id);
        assert_eq!(value_of(&argv(&p), "--resume"), Some(id.as_str()));
    }

    #[test]
    fn other_control_requests_get_an_error_answer() {
        let (_env, root) = crate::paths::testenv::folders("chat-control");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let other = json!({"type": "control_request", "request_id": "req-9", "request": {"subtype": "mcp_message", "server_name": "x"}});
        script(&p, "chat.1.jsonl", &format!("{other}\n{}", turn("Fine.")));
        let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
        let ev = turn_events(&v.id, &id);
        assert!(of_type(&ev, "permission").is_empty() && of_type(&ev, "result").len() == 1, "{ev:#?}");
        let answer: Value = file(&p, "chat.in.jsonl").lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|l| l["type"] == "control_response").unwrap();
        assert_eq!(answer["response"]["subtype"], "error");
        assert_eq!(answer["response"]["request_id"], "req-9");
    }

    #[test]
    fn three_live_and_idle_ones_stop() {
        let (_env, root) = crate::paths::testenv::folders("chat-three");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.jsonl", &turn("Noted."));
        let start = |q: &str| {
            let id = send(&v.id, None, q, Mode::Ask, &[]).unwrap();
            turn_events(&v.id, &id);
            id
        };
        let (a, b, c) = (start("one"), start("two"), start("three"));
        assert_eq!(live().len(), 3);
        // Ten minutes later a fourth starts: the longest idle stops first (D38).
        skew(Duration::from_secs(600));
        let d = start("four");
        until("the longest idle stopped", || !live().contains(&a));
        let l = live();
        assert!(l.contains(&b) && l.contains(&c) && l.contains(&d), "{l:?}");
        // Idle ten minutes: stopped; the one just used stays.
        sweep();
        until("the idle ones stopped", || live() == vec![d.clone()]);
        // Stopped ones resume.
        send(&v.id, Some(&a), "one again", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &a);
        assert_eq!(value_of(&argv(&p), "--resume"), Some(a.as_str()));
    }

    #[test]
    fn a_fourth_is_refused_while_three_answer() {
        // Final review F4: idle ones make room (D38); one answering is never stopped mid-answer for another.
        let (_env, root) = crate::paths::testenv::folders("chat-three-busy");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.jsonl", "{\"_hang\":true}\n");
        let ids: Vec<String> = ["one", "two", "three"].iter().map(|q| send(&v.id, None, q, Mode::Ask, &[]).unwrap()).collect();
        let answering = || ids.iter().all(|id| read(&v.id, id, 0, Duration::ZERO).is_some_and(|c| c.live && c.busy));
        until("three answering", answering);
        // Long since they were sent: still answering, so none is the one to stop.
        skew(IDLE);
        let e = send(&v.id, None, "four", Mode::Ask, &[]).unwrap_err().to_string();
        assert_eq!(e, "3 conversations are answering. Wait for one, or stop one.");
        std::thread::sleep(Duration::from_millis(300));
        assert!(answering(), "none of the three is stopped");
        assert_eq!(live().len(), 3);
        assert_eq!(file(&p, "pids").lines().count(), 3, "no fourth process");
        assert!(list_value(&v).as_array().unwrap().iter().all(|c| ids.iter().any(|id| c["id"] == id.as_str())), "no fourth conversation left behind");
    }

    #[test]
    fn a_conversation_live_in_the_panel_is_refused() {
        let (_env, root) = crate::paths::testenv::folders("chat-panel");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let store = crate::terminal::vault_store_key(&v);
        crate::terminal::remember(&store, ID);
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "sleep 30"]);
        let pty = crate::terminal::open_with("claude:vault", cmd, 80, 20, None, None, Some(ID.into())).unwrap();
        let e = send(&v.id, Some(ID), "Hello", Mode::Ask, &[]).unwrap_err().to_string();
        assert!(e.contains("open in the terminal panel"), "{e}");
        crate::terminal::close(pty);
        // And the panel refuses one live in Ask.
        script(&p, "chat.jsonl", &turn("Hi."));
        let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        let e = crate::terminal::start_vault("claude:vault:twin", &v.id, Some(&id), 80, 20).unwrap_err().to_string();
        assert!(e.contains("open in Ask"), "{e}");
        // Taken to the terminal panel from Ask: its process ends first; the line resumes it (a managed profile has none).
        let t = to_terminal(&v.id, &id).unwrap();
        assert_eq!(t["conversation"], id.as_str());
        assert!(t["command"].is_null());
        assert!(!live().contains(&id));
    }

    #[test]
    fn a_crash_mid_turn_ends_the_turn_with_an_error() {
        let (_env, root) = crate::paths::testenv::folders("chat-crash");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let half: String = turn("Half an ans").lines().take(3).map(|l| format!("{l}\n")).collect();
        script(&p, "chat.1.jsonl", &format!("{half}{{\"_exit\":3}}\n"));
        let id = send(&v.id, None, "What is open with Acme?", Mode::Ask, &[]).unwrap();
        let ev = events_until(&v.id, &id, |_, c| !c.busy && !c.live);
        let last = ev.last().unwrap();
        assert_eq!(last["type"], "error", "{ev:#?}");
        assert!(last["text"].as_str().unwrap().contains("fake claude crashed"), "{last}");
        assert!(of_type(&ev, "text").last().is_some_and(|t| t["done"] == true), "the partial text is closed: {ev:#?}");
        script(&p, "chat.2.jsonl", &turn("Here it is."));
        send(&v.id, Some(&id), "Again", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        assert_eq!(value_of(&argv(&p), "--resume"), Some(id.as_str()), "still resumable");
    }

    #[test]
    fn a_sign_in_needed_is_an_error_event_with_its_action() {
        let (_env, root) = crate::paths::testenv::folders("chat-signin-needed");
        let _fake = fake::install(&root);
        let _clean = clean();
        let _p = fake::profile("w", None, fake::SIGNED_OUT);
        let v = vault(&root, "w");
        let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
        let ev = events_until(&v.id, &id, |ev, _| !ev.is_empty());
        assert_eq!(ev[0]["type"], "error");
        assert_eq!(ev[0]["action"], "signin");
        assert_eq!(ev[0]["profile"], "w");
        assert!(live().is_empty());
    }

    #[test]
    fn a_first_start_that_dies_before_its_session_starts_new_again() {
        let (_env, root) = crate::paths::testenv::folders("chat-first-dies");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        // Claude Code refused at once (a flag it doesn't know): no `init`, no session.
        script(&p, "chat.1.jsonl", "{\"_exit\":3}\n");
        let id = send(&v.id, None, "What is open with Acme?", Mode::Ask, &[]).unwrap();
        events_until(&v.id, &id, |_, c| !c.busy && !c.live);
        script(&p, "chat.2.jsonl", &turn("Here it is."));
        send(&v.id, Some(&id), "What is open with Acme?", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        let a = argv(&p);
        assert_eq!(value_of(&a, "--session-id"), Some(id.as_str()), "{a:?}");
        assert!(value_of(&a, "--resume").is_none(), "there is nothing to resume: {a:?}");
    }

    #[test]
    fn the_tests_sweep_by_themselves() {
        let (_env, root) = crate::paths::testenv::folders("chat-no-sweeper");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.jsonl", &turn("Noted."));
        let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        // A sweep of its own could fire between a test's clock skew and its check.
        assert!(!SWEEPER.is_completed(), "no sweeper thread in tests");
    }

    #[test]
    fn quitting_stops_chats_at_once() {
        let (_env, root) = crate::paths::testenv::folders("chat-quit");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.1.jsonl", "{\"_hang\":true}\n");
        let id = send(&v.id, None, "Summarise Northwind Retail", Mode::Ask, &[]).unwrap();
        until("the turn going", || file(&p, "chat.in.jsonl").contains("Northwind"));
        // Echo quits right after: only what `close_all` does before it returns counts.
        NO_ESCALATION.store(true, Ordering::SeqCst);
        close_all();
        until("its process stopped", || gone(&file(&p, "pids")));
        events_until(&v.id, &id, |_, c| !c.live);
    }

    #[test]
    fn a_stop_whose_writes_block_still_ends_the_process() {
        let (_env, root) = crate::paths::testenv::folders("chat-stop-blocked");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.1.jsonl", "{\"_hang\":true}\n");
        let id = send(&v.id, None, "Summarise Northwind Retail", Mode::Ask, &[]).unwrap();
        let c = lock(&CHATS).get(&id).cloned().unwrap();
        let stdin = lock(&c.state).proc.as_ref().unwrap().stdin.clone();
        // Its input is busy (a write that doesn't get through): Stop's own writes wait.
        let held = lock(&stdin);
        let stopping = { let id = id.clone(); std::thread::spawn(move || stop(&id)) };
        std::thread::sleep(Duration::from_millis(200));
        skew(Duration::from_secs(5));
        until("the stop's 5 s ended it", || gone(&file(&p, "pids")));
        drop(held);
        stopping.join().unwrap();
        let ev = events_until(&v.id, &id, |_, c| !c.busy && !c.live);
        assert_eq!(ev.last().unwrap()["kind"], "stopped");
    }

    #[test]
    fn a_block_rendered_many_times_keeps_one_copy() {
        let (_env, root) = crate::paths::testenv::folders("chat-one-copy");
        let v = vault(&root, "system");
        let c = Conv::new(ID, &v, false);
        let mut st = lock(&c.state);
        let long = "Northwind Retail fixes forward. ".repeat(200);
        for i in 1..=50 {
            st.push(json!({"type": "text", "block": "b1", "html": format!("<p>{}</p>", &long[..long.len() * i / 50]), "done": false}));
            st.push(json!({"type": "thinking", "block": "b2", "text": &long[..long.len() * i / 50], "done": false}));
        }
        st.push(json!({"type": "text", "block": "b1", "html": format!("<p>{long}</p>"), "done": true}));
        assert_eq!(st.events.iter().filter(|e| e.get("html").is_some()).count(), 1, "the latest render only");
        assert_eq!(st.events.iter().filter(|e| e["type"] == "thinking" && e.get("text").is_some()).count(), 1);
        assert_eq!(st.events.back().unwrap()["html"].as_str().unwrap().len(), long.len() + 7);
        assert!(st.events.iter().enumerate().all(|(i, e)| e["n"] == i as u64), "numbers stay as they were");
        assert!(st.events[0]["superseded"] == true);
        // A new process numbers its blocks again from b1: its first render supersedes nothing of the last one's.
        st.renders.clear();
        st.push(json!({"type": "text", "block": "b1", "html": "<p>Again</p>", "done": false}));
        assert!(st.events[100].get("html").is_some());
    }

    #[test]
    fn the_chats_route_lists_the_vaults_conversations() {
        let (_env, root) = crate::paths::testenv::folders("chat-list");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        script(&p, "chat.jsonl", &turn("Noted."));
        let id = send(&v.id, None, "What is new at Northwind Retail?", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        let l = list_value(&v);
        let mine = l.as_array().unwrap().iter().find(|c| c["id"] == id.as_str()).expect("listed");
        assert_eq!(mine["live"], true);
        assert_eq!(mine["busy"], false);
        assert!(mine["at"].is_string());
    }

    /// `body` as conversation `ID`'s transcript in the profile's Claude Code folder, listed under the vault's key.
    fn past(p: &ai::Profile, v: &Vault, body: &str) {
        let dir = p.config_folder().join("projects/-made-up");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{ID}.jsonl")), body).unwrap();
        crate::terminal::remember(&crate::terminal::vault_store_key(v), ID);
    }

    #[test]
    fn a_past_conversation_opens_from_its_transcript_and_continues() {
        let (_env, root) = crate::paths::testenv::folders("chat-history");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        past(&p, &v, &include_str!("testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
        // No process and no events: the page reads the transcript, numbered from 0 as live events are.
        let c = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        let t: Vec<&str> = c.events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(t, ["user", "tool", "tool_result", "thinking", "text"], "{:#?}", c.events);
        assert_eq!(c.events.iter().map(|e| e["n"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
        assert!((c.next, c.live, c.busy) == (5, false, false));
        // Read again: not twice.
        assert_eq!(read(&v.id, ID, 5, Duration::from_millis(50)).unwrap().events.len(), 0);
        // Continuing it: the live events follow, the history keeps its numbers.
        script(&p, "chat.jsonl", &turn("Fixed."));
        assert_eq!(send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap(), ID);
        let live = events_until(&v.id, ID, |ev, c| !c.busy && of_type(ev, "result").len() == 1);
        assert_eq!(live[..5], c.events[..], "the history is as it was");
        assert_eq!(live[5]["type"], "user");
        assert_eq!(live[5]["n"], 5);
        assert!(live.iter().skip(5).all(|e| e["n"].as_u64().unwrap() >= 5));
        assert_eq!(value_of(&argv(&p), "--resume"), Some(ID));
    }

    #[test]
    fn an_idle_conversation_lets_its_events_go_and_reads_them_again() {
        // Final review F5: a conversation opened from the list doesn't keep its events for the app's life.
        let (_env, root) = crate::paths::testenv::folders("chat-let-go");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        past(&p, &v, &include_str!("testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
        let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        assert_eq!((first.events.len(), first.next), (5, 5));
        let held = || lock(&CHATS).get(ID).map(|c| lock(&c.state).events.len());
        // Ten minutes idle with no process: its events go from memory.
        skew(IDLE);
        sweep();
        assert_eq!(held(), Some(0), "its events are let go");
        // A page still showing it polls on: nothing new, nothing twice.
        let c = read(&v.id, ID, 5, Duration::from_millis(50)).unwrap();
        assert!(c.events.is_empty() && c.next == 5, "{:#?}", c.events);
        // Opened again: the same events from its transcript, numbered after every number it gave (5 was its end).
        let again = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        let words = |ev: &[Value]| ev.iter().map(|e| e.as_object().unwrap().iter().filter(|(k, _)| *k != "n").map(|(k, x)| (k.clone(), x.clone())).collect::<Vec<_>>()).collect::<Vec<_>>();
        assert_eq!(words(&again.events), words(&first.events));
        assert_eq!(again.events.iter().map(|e| e["n"].as_u64().unwrap()).collect::<Vec<_>>(), [6, 7, 8, 9, 10]);
        assert_eq!(again.next, 11);
        assert!(read(&v.id, ID, 11, Duration::from_millis(50)).unwrap().events.is_empty(), "and the poll goes on from there");
        // Let go again while a page shows it: a message from that page follows what it shows, its past not read twice.
        skew(IDLE);
        sweep();
        assert_eq!(held(), Some(0));
        script(&p, "chat.jsonl", &turn("Fixed."));
        send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap();
        let (ev, _) = poll_from(&v, 11);
        assert_eq!((ev[0]["type"].as_str(), ev[0]["n"].as_u64()), (Some("user"), Some(11)), "{ev:#?}");
        assert!(ev.iter().all(|e| e["n"].as_u64().unwrap() >= 11));
        // A conversation going on keeps its events.
        skew(IDLE);
        sweep();
        until("its idle process ended", || !live().contains(&ID.to_string()));
        assert!(held().unwrap() > 0, "its process has only just ended");
    }

    /// The page's poll from `from` until the turn has ended: its events and where it is then.
    fn poll_from(v: &Vault, mut from: u64) -> (Vec<Value>, u64) {
        let (mut all, t) = (vec![], Instant::now());
        loop {
            let c = read(&v.id, ID, from, Duration::from_millis(200)).expect("the conversation");
            from = c.next;
            all.extend(c.events.iter().cloned());
            if !c.busy && all.iter().any(|e| e["type"] == "result") {
                return (all, from);
            }
            assert!(t.elapsed() < Duration::from_secs(10), "timed out: {all:#?}");
        }
    }

    /// The user's words and the finished answers' text in `ev`, in order: what a reader sees of the conversation.
    fn said(ev: &[Value]) -> Vec<String> {
        ev.iter()
            .filter_map(|e| match e["type"].as_str() {
                Some("user") => e["text"].as_str().map(|t| format!("you: {}", t.chars().take(30).collect::<String>())),
                Some("text") if e["done"] == true => e["md"].as_str().map(|t| format!("claude: {}", t.chars().take(30).collect::<String>())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_conversation_continued_after_a_let_go_opens_whole_again() {
        // Final review X2: opened from before what Echo holds, it is read again from its transcript — the earlier
        // turns and the new one, each once — and a page that kept polling takes it whole once more, not twice.
        let (_env, root) = crate::paths::testenv::folders("chat-reopen-whole");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        past(&p, &v, &include_str!("testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
        let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        let before = said(&first.events);
        assert_eq!(before.len(), 2, "{before:?}");
        skew(IDLE);
        sweep();
        // The page still showing it sends a follow-up and polls on from where it was.
        script(&p, "chat.jsonl", &turn("Fixed, with its date and tags."));
        send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap();
        let (shown, at) = poll_from(&v, first.next);
        assert_eq!(said(&shown), ["you: Yes, fix it", "claude: Fixed, with its date and tags."]);
        // Claude Code wrote the turn into its transcript (the fake doesn't: done here, as it would be).
        let file = p.config_folder().join(format!("projects/-made-up/{ID}.jsonl"));
        let turn_lines = [
            json!({"type": "user", "isSidechain": false, "message": {"role": "user", "content": [{"type": "text", "text": "Yes, fix it"}]}}),
            json!({"type": "assistant", "isSidechain": false, "message": {"id": "msg_follow", "type": "message", "role": "assistant", "content": [{"type": "text", "text": "Fixed, with its date and tags."}], "stop_reason": "end_turn"}}),
        ];
        let mut t = std::fs::read_to_string(&file).unwrap();
        turn_lines.iter().for_each(|l| t.push_str(&format!("{l}\n")));
        std::fs::write(&file, t).unwrap();
        // Opened again (another conversation and back, a reload): the whole of it, each message once.
        let again = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        let whole = [before.clone(), said(&shown)].concat();
        assert_eq!(said(&again.events), whole);
        // The page that kept polling: its next answer doesn't follow on from where it was (so it takes it whole), and
        // it is the whole conversation, each message once; then nothing more.
        let kept = read(&v.id, ID, at, Duration::from_millis(50)).unwrap();
        assert_ne!(kept.events.first().and_then(|e| e["n"].as_u64()), Some(at), "it can't pass for what follows");
        assert_eq!(said(&kept.events), whole);
        assert!(read(&v.id, ID, kept.next, Duration::from_millis(50)).unwrap().events.is_empty());
    }

    #[test]
    fn opened_while_a_turn_goes_it_is_read_whole_when_the_turn_ends() {
        // Final review X2: a page opening it mid-turn after a let-go gets the whole conversation once the turn is over.
        let (_env, root) = crate::paths::testenv::folders("chat-reopen-busy");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let history = include_str!("testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy());
        past(&p, &v, &history);
        let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        skew(IDLE);
        sweep();
        // Its turn writes the transcript (as Claude Code does), then waits on a card.
        let follow = [
            json!({"type": "user", "isSidechain": false, "message": {"role": "user", "content": [{"type": "text", "text": "Yes, fix it"}]}}),
            json!({"type": "assistant", "isSidechain": false, "message": {"id": "msg_follow", "type": "message", "role": "assistant", "content": [{"type": "text", "text": "Fixed."}], "stop_reason": "end_turn"}}),
        ];
        let file = p.config_folder().join(format!("projects/-made-up/{ID}.jsonl"));
        std::fs::write(fake::file(&p, "apply.1"), format!("{history}{}", follow.iter().map(|l| format!("{l}\n")).collect::<String>())).unwrap();
        std::fs::write(fake::file(&p, "apply.1.to"), file.display().to_string()).unwrap();
        let asks = json!({"type": "control_request", "request_id": "req-1", "request": {"subtype": "can_use_tool", "tool_name": "Edit", "input": {"file_path": format!("{}/knowledge/clients/acme.md", v.root.display()), "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nFixed.\n"}}});
        let played = turn("Fixed.");
        let (init, rest) = played.split_once('\n').unwrap();
        script(&p, "chat.jsonl", &format!("{init}\n{{\"_apply\":1}}\n{asks}\n{rest}"));
        send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap();
        until("the card", || read(&v.id, ID, first.next, Duration::ZERO).is_some_and(|c| c.waiting));
        // Opened meanwhile: what it holds; the transcript isn't read while the turn goes.
        let opened = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        assert_eq!(said(&opened.events), ["you: Yes, fix it"]);
        permit(ID, "req-1", false, false, None).unwrap();
        // That page polls on: once the turn is over, the whole conversation, each message once.
        let (mut from, mut seen, t) = (opened.next, vec![], Instant::now());
        loop {
            let c = read(&v.id, ID, from, Duration::from_millis(200)).unwrap();
            if c.events.first().and_then(|e| e["n"].as_u64()).is_some_and(|n| n != from) {
                seen.clear(); // answered from elsewhere than asked: the page takes it whole again
            }
            from = c.next;
            seen.extend(c.events.iter().cloned());
            if !c.busy && said(&seen).len() == 4 {
                break;
            }
            assert!(t.elapsed() < Duration::from_secs(10), "{seen:#?}");
        }
        assert_eq!(said(&seen)[2..], ["you: Yes, fix it".to_string(), "claude: Fixed.".to_string()]);
        assert_eq!(said(&seen)[..2], said(&first.events)[..]);
    }

    #[test]
    fn a_sweep_right_after_opening_drops_nothing() {
        // Final review X2: opening counts as use.
        let (_env, root) = crate::paths::testenv::folders("chat-open-use");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        past(&p, &v, &include_str!("testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
        let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        skew(IDLE);
        sweep();
        let held = || lock(&CHATS).get(ID).map(|c| lock(&c.state).events.len());
        assert_eq!(held(), Some(0));
        // Opened again, and a sweep comes at once: what was just read stays for the page.
        let again = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        sweep();
        assert_eq!(held(), Some(first.events.len()), "nothing let go right after it was opened");
        assert_eq!(said(&read(&v.id, ID, 0, Duration::from_millis(50)).unwrap().events), said(&again.events));
    }

    #[test]
    fn a_transcript_that_cant_be_read_is_said_in_the_conversation() {
        let (_env, root) = crate::paths::testenv::folders("chat-history-broken");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        past(&p, &v, "this is not a transcript\n");
        let c = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
        assert_eq!(c.events.len(), 1, "{:#?}", c.events);
        assert_eq!((c.events[0]["type"].as_str(), c.events[0]["n"].as_u64()), (Some("notice"), Some(0)));
        assert_eq!(c.events[0]["text"], "Its history can't be shown here; continuing works.");
        // A conversation never started has no past to show.
        script(&p, "chat.jsonl", &turn("Hi."));
        let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
        assert!(turn_events(&v.id, &id).iter().all(|e| e["type"] != "notice"));
    }
}
