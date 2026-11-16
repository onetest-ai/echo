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
mod tests;
