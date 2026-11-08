//! Signing a `claude` profile in from the app (the window, the tray): Echo runs `claude auth
//! login` for the profile's own folder itself — no terminal; the browser opens on Claude's
//! sign-in page and calls Claude Code back on this computer — and notices when it is done.
//!
//! - `start` (Sign in…): the profile's exclusive lease (`ai::sign_in_lease`; the app's live jobs on
//!   it are held back meanwhile — `livenotes::defer` — and others refused: `ai::SigningIn`), the
//!   login child, then one `auth status` check judged against the settings as they are now
//!   (`ai::settle`). Completion is the child's exit: it ends by itself after the browser's
//!   callback (measured). 10 minutes at most.
//! - `ensure_request` (after `choose`: a profile chosen in one of the window's menus; `ensure` in
//!   tests): a check first; only an explicit
//!   "signed out" opens the browser, and only while one of the choices it is for is still the
//!   latest of its purpose (`call:<name>`, `record`, `notes:<name>`) — checked when the check ends
//!   and again right before the browser opens; never for an error or another account. Choosing the
//!   same profile elsewhere meanwhile joins the check going on. A call's choice counts from the
//!   moment it reaches the app (`choose`), its check starts once it was applied (`applied`).
//! - `state`: what the window and the tray show, from the flow and the last observation
//!   (`ai::observation`); it never runs a process (a background check when nothing current is known).
//! - `recovered` (signed in, an account accepted, or a background check finding it usable again):
//!   live notes waiting for it go on at once.
//! - `shutdown`: every flow and check is stopped, its child reaped, nothing started after.
//!
//! `echoctl ai login` signs in in the terminal it was run from instead (`ai::login_here`), under the
//! same lock; the window learns of it from the lock's token (a background check).

use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::ai::{self, Profile};
use crate::paths;

/// How long a sign-in may take in the browser.
const DEADLINE: Duration = Duration::from_secs(600);
/// A background check of a profile at most this often (unless asked for).
const REFRESH_EVERY: Duration = Duration::from_secs(30);

/// A Claude profile's sign-in as the window (`ai::summary` → `p.signin`, ui/app/10-shell.js `signinNote`)
/// and the tray show it. `account` is an `Account::key` (`Use <account>` sends it back to `accept`),
/// with the plan (`label`) only right after a sign-in.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SignInState {
    /// A sign-in is getting ready (live jobs on the profile stopping).
    Starting,
    /// `ensure_request`'s check runs.
    Checking,
    /// The browser is open; the login child waits for its callback.
    Waiting { email: Option<String> },
    /// Cancelled, its child being stopped.
    Stopping,
    /// A sign-in outside the app holds the lock (`echoctl ai login` in a terminal).
    Elsewhere,
    SignedIn {
        account: String,
        /// Signed in by a sign-in that just ended ("Signed in as …").
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        just: bool,
    },
    OtherAccount { account: String, expected: String },
    SignedOut,
    /// The last sign-in didn't end signed in.
    Failed { error: String },
    /// Not known (yet): a check is due or failed.
    Unknown { error: Option<String> },
}

#[derive(Clone, Debug, PartialEq)]
enum Outcome {
    /// The account's label (with the plan).
    SignedIn(String),
    OtherAccount { now: String, expected: String },
    Failed(String),
    TimedOut,
}

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Starting,
    Checking,
    Waiting,
    /// `seen`: the last observation's seq when it ended — a newer one supersedes the outcome; so
    /// does another sign-in token (`echoctl ai login` since).
    Done { outcome: Outcome, seen: u64, token: Option<String> },
}

#[derive(Clone, Debug)]
struct Flow {
    generation: u64,
    phase: Phase,
    since: Instant,
    cancel: bool,
    email: Option<String>,
    /// For menu choices (`ensure_request`): the (purpose, request) pairs it is for — it goes on while one
    /// of them is still the latest of its purpose. None: Sign in… (asked for, whatever the menus say).
    choices: Option<Vec<(String, u64)>>,
    /// A choice joined since it last decided (`again`): before giving up, it decides again.
    dirty: bool,
}

impl Flow {
    /// Still asked for: Sign in…, or one of its menu choices is still the latest of its purpose.
    fn wanted(&self, purposes: &HashMap<String, (u64, String)>) -> bool {
        self.choices.as_ref().is_none_or(|c| c.iter().any(|(purpose, request)| purposes.get(purpose).map(|(n, _)| *n) == Some(*request)))
    }
}

#[derive(Default)]
struct Shared {
    flows: HashMap<String, Flow>,
    /// The latest choice of each purpose: its request number and the profile chosen.
    purposes: HashMap<String, (u64, String)>,
    /// Background checks: when the last one started, and whether it still runs.
    refreshing: HashMap<String, (Instant, bool)>,
    /// Threads that may have a child of theirs (flows, checks, accepts): `shutdown` waits for them.
    active: usize,
    /// Profiles an in-app sign-in holds the exclusive lease of (until it is released), and how
    /// many times one released it: a check refused by that lock — held then, or let go since the
    /// check began — waits for it and looks again, rather than taking it for a sign-in elsewhere.
    holding: std::collections::HashSet<String>,
    released: HashMap<String, u64>,
    shutdown: bool,
    next: u64,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    let mut g = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Shared::default))
}

fn profile(id: &str) -> Result<Profile> {
    ai::Settings::load()?.get(id)
}

/// The flow of `id` with this generation is still the one to run (not cancelled, replaced, or
/// stopped by the app quitting).
fn current(id: &str, generation: u64) -> bool {
    with(|s| !s.shutdown && s.flows.get(id).is_some_and(|f| f.generation == generation && !f.cancel))
}

/// The flow's outcome, with the observation and token it saw, in one step. True when set; a flow
/// cancelled meanwhile is cleared instead (its state is then the folder's again), a replaced one
/// left to its successor.
fn done(p: &Profile, generation: u64, outcome: Outcome) -> bool {
    let seen = ai::observation(&p.id).map_or(0, |o| o.seq);
    let token = ai::token_now(p);
    #[cfg(test)]
    if let Some(hook) = *BEFORE_DONE.lock().unwrap_or_else(|e| e.into_inner()) {
        hook();
    }
    with(|s| {
        let stopped = s.shutdown;
        match s.flows.get_mut(&p.id).filter(|f| f.generation == generation) {
            Some(f) if !f.cancel && !stopped => {
                f.phase = Phase::Done { outcome, seen, token };
                f.since = Instant::now();
                true
            }
            Some(_) => {
                s.flows.remove(&p.id);
                false
            }
            None => false,
        }
    })
}

/// (tests) Runs in `done` between reading what it records and recording it.
#[cfg(test)]
static BEFORE_DONE: Mutex<Option<fn()>> = Mutex::new(None);

/// Drop the flow of `id` if it is still this one (its state then comes from the observation).
fn clear(id: &str, generation: u64) {
    with(|s| {
        if s.flows.get(id).is_some_and(|f| f.generation == generation) {
            s.flows.remove(id);
        }
    });
}

/// A new flow for `id`. Sign in… (`choice` None) replaces, and so stops, the one there was. A menu
/// `choice` (purpose, request) counts only while it is the latest of its purpose (checked under
/// this lock — a delayed older one changes nothing), and joins a flow going on (a check, a
/// sign-in) instead of replacing it. None: quitting, not the latest, or joined.
fn new_flow(id: &str, p: &Profile, phase: Phase, choice: Option<(&str, u64)>) -> Option<u64> {
    with(|s| {
        if s.shutdown {
            return None;
        }
        if let Some((purpose, request)) = choice {
            if s.purposes.get(purpose).map(|(n, _)| *n) != Some(request) {
                return None;
            }
            if let Some(f) = s.flows.get_mut(id).filter(|f| !f.cancel && !matches!(f.phase, Phase::Done { .. })) {
                if let Some(c) = f.choices.as_mut() {
                    c.push((purpose.to_string(), request));
                    f.dirty = true;
                }
                return None;
            }
        }
        s.next += 1;
        let generation = s.next;
        let choices = choice.map(|(purpose, request)| vec![(purpose.to_string(), request)]);
        s.flows.insert(id.to_string(), Flow { generation, phase, since: Instant::now(), cancel: false, email: ai::login_email(p), choices, dirty: false });
        s.active += 1;
        Some(generation)
    })
}

/// The check of flow `generation` should stop: not current, or no longer asked for — then it is
/// marked cancelled in the same step, so a choice made from now on starts a new flow instead of
/// joining one that is stopping.
fn abandoned(id: &str, generation: u64) -> bool {
    with(|s| {
        let Shared { flows, purposes, shutdown, .. } = s;
        match flows.get_mut(id) {
            Some(f) if f.generation == generation && !f.cancel && !*shutdown => {
                if !f.wanted(purposes) {
                    f.cancel = true;
                }
                f.cancel
            }
            _ => true,
        }
    })
}

/// Its menu choices that still count also still hold on disk: a call's choice only while the call
/// still uses the profile (the switch was applied, and not changed since).
fn still_chosen(id: &str, generation: u64) -> bool {
    // (Whether it came from a menu, and its choices that still count: from the same flow, at once.)
    let valid: Option<(bool, Vec<String>)> = with(|s| {
        let f = s.flows.get(id).filter(|f| f.generation == generation)?;
        Some(match &f.choices {
            None => (false, vec![]),
            Some(c) => (true, c.iter().filter(|(purpose, request)| s.purposes.get(purpose).map(|(n, _)| *n) == Some(*request)).map(|(p, _)| p.clone()).collect()),
        })
    });
    match valid {
        None => false,
        Some((false, _)) => true,
        Some((true, v)) => v.iter().any(|purpose| match purpose.strip_prefix("call:") {
            // The call by name, else by stamp (retitled since it was chosen).
            Some(call) => crate::calls::resolve_call(call)
                .ok()
                .or_else(|| crate::calls::by_stamp(&crate::calls::stamp_of(std::path::Path::new(call))))
                .and_then(|d| ai::call_profile_id(&d).ok())
                .as_deref()
                == Some(id),
            None => true,
        }),
    }
}

/// A thread counted in `active` until it ends (a panic too).
struct Active;

impl Drop for Active {
    fn drop(&mut self) {
        with(|s| s.active = s.active.saturating_sub(1));
    }
}

/// Sign `id` in now (Sign in…, the tray): a flow already going on is stopped and replaced (the
/// tab was closed, or another account is wanted).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn start(id: &str) {
    let p = match profile(id) {
        Ok(p) if p.kind.signs_in() => p,
        Ok(p) => return crate::app::notify("Nothing to sign in", &format!("{} doesn't sign in to an account", p.name)),
        Err(e) => return crate::app::notify("Couldn't sign in", &format!("{e:#}")),
    };
    let Some(generation) = new_flow(id, &p, Phase::Starting, None) else { return };
    std::thread::spawn(move || {
        let _active = Active;
        run(&p, generation);
    });
}

/// Profile `id` was chosen for `purpose`: any choice (of any kind of profile) supersedes the
/// purpose's earlier ones — their checks open nothing any more. The request number.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn choose(purpose: &str, id: &str) -> u64 {
    with(|s| {
        s.next += 1;
        s.purposes.insert(purpose.to_string(), (s.next, id.to_string()));
        s.next
    })
}

/// A profile was chosen in a menu: sign it in if — and only if — it is plainly signed out (the
/// window registers the choice with `choose` on its own thread, in the order they came, then calls
/// `ensure_request`).
#[cfg(test)]
pub fn ensure(id: &str, purpose: &str) {
    let request = choose(purpose, id);
    ensure_request(id, purpose, request);
}

/// The call's profile switch to `id` was applied: if it is still the latest choice for the call
/// (only choices made in the window count: `choose`), sign `id` in when it needs it.
pub fn applied(call: &str, id: &str) {
    let purpose = format!("call:{call}");
    let request = with(|s| s.purposes.get(&purpose).filter(|(_, chosen)| chosen == id).map(|(n, _)| *n));
    if let Some(request) = request {
        ensure_request(id, &purpose, request);
    }
}

/// The check for choice `request` of `purpose` (`choose`): signed out and still the latest choice
/// → the sign-in starts.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn ensure_request(id: &str, purpose: &str, request: u64) {
    if with(|s| s.shutdown) {
        return;
    }
    let Ok(p) = profile(id) else { return }; // removed meanwhile: nothing to sign in
    if !p.kind.signs_in() || fresh_and_fine(&p) {
        return;
    }
    let Some(generation) = new_flow(id, &p, Phase::Checking, Some((purpose, request))) else { return };
    std::thread::spawn(move || {
        let _active = Active;
        let cancelled = || abandoned(&p.id, generation);
        loop {
            let releases = with(|s| s.released.get(&p.id).copied().unwrap_or(0));
            let seen = ai::observe(&p, &cancelled);
            #[cfg(test)]
            if seen.as_ref().is_err_and(|e| e.downcast_ref::<ai::SigningIn>().is_some()) {
                if let Some(hook) = *AFTER_REFUSED.lock().unwrap_or_else(|e| e.into_inner()) {
                    hook();
                }
            }
            if cancelled() {
                return clear(&p.id, generation);
            }
            match seen {
                Ok(None) if still_chosen(&p.id, generation) => return run(&p, generation),
                Ok(Some(a)) => match ai::settle(&p, Some(&a)) {
                    Ok(_) => {} // signed in (the observation says so)
                    Err(e) => {
                        let outcome = match other_account(&e) {
                            Some((now, expected)) => Outcome::OtherAccount { now, expected },
                            None => Outcome::Failed(format!("{e:#}")),
                        };
                        if done(&p, generation, outcome.clone()) {
                            say(&p, &outcome);
                        }
                        return;
                    }
                },
                // Refused by the lock of an in-app sign-in that is ending (cancelled, replaced) — or
                // let go of since this check began: wait for it, then look again.
                Err(e) if e.downcast_ref::<ai::SigningIn>().is_some() && own_sign_in_ending(&p.id, releases, &cancelled) => continue,
                // No longer chosen, a sign-in elsewhere, or a check error (the observation shows
                // it): no browser.
                _ => {}
            }
            if !again(&p.id, generation) {
                return;
            }
        }
    });
}

/// Give up flow `generation` of `id` — unless a choice joined it since it last decided (`dirty`):
/// then it decides again (true). Giving up retires it in the same step (cancelled — nothing joins
/// it any more — and removed), so a choice made from then on starts a flow of its own.
fn again(id: &str, generation: u64) -> bool {
    #[cfg(test)]
    if let Some(hook) = *BEFORE_GIVING_UP.lock().unwrap_or_else(|e| e.into_inner()) {
        hook();
    }
    with(|s| {
        let Some(f) = s.flows.get_mut(id).filter(|f| f.generation == generation) else { return false };
        if f.dirty && !f.cancel && !s.shutdown {
            f.dirty = false;
            return true;
        }
        s.flows.remove(id);
        false
    })
}

/// An in-app sign-in of `id` holds its lock and is ending, or let go of it since `releases` (the
/// count when the check began): wait (≤ 15 s, or until `cancelled`) for it to let go. False: none
/// (the lock is held elsewhere — `echoctl ai login`).
fn own_sign_in_ending(id: &str, releases: u64, cancelled: &dyn Fn() -> bool) -> bool {
    let t = Instant::now();
    let mut ours = with(|s| s.released.get(id).copied().unwrap_or(0) != releases);
    while with(|s| s.holding.contains(id)) && t.elapsed() < Duration::from_secs(15) && !cancelled() {
        ours = true;
        std::thread::sleep(Duration::from_millis(100));
    }
    ours
}

/// (tests) Runs when a check was refused by the lock, before it asks whose it is.
#[cfg(test)]
static AFTER_REFUSED: Mutex<Option<fn()>> = Mutex::new(None);

/// An in-app sign-in's exclusive lease, marked in `Shared.holding` until it is released (the lease
/// first, then the mark — a panic too).
struct Holding {
    id: String,
    lease: Option<ai::Lease>,
}

impl Drop for Holding {
    fn drop(&mut self) {
        self.lease.take();
        with(|s| {
            s.holding.remove(&self.id);
            *s.released.entry(self.id.clone()).or_default() += 1;
        });
    }
}

/// (tests) Runs where a flow has decided to give up, before `again` looks at it.
#[cfg(test)]
static BEFORE_GIVING_UP: Mutex<Option<fn()>> = Mutex::new(None);

/// The folder is signed in to an account jobs may use, as last seen (not stale, same token), and
/// no model refused it since (`ai::refused_by_model`: `auth status` alone doesn't undo that).
fn usable(p: &Profile, o: &ai::Observation) -> bool {
    matches!(&o.result, Ok(Some(a)) if p.expects(a)) && !o.stale && ai::token_now(p).as_deref() == Some(o.token.as_str()) && !ai::refused_by_model(p)
}

/// Usable, seen less than a minute ago.
fn fresh_and_fine(p: &Profile) -> bool {
    ai::observation(&p.id).is_some_and(|o| usable(p, &o) && o.at.elapsed() < Duration::from_secs(60))
}

fn other_account(e: &anyhow::Error) -> Option<(String, String)> {
    e.downcast_ref::<ai::SignInNeeded>().and_then(|s| Some((s.now.clone()?, s.expected.clone()?)))
}

/// Holds `livenotes::defer`'s hold until dropped — when the sign-in is over, a panic included.
struct Deferral<'a>(&'a str, u64);

impl Drop for Deferral<'_> {
    fn drop(&mut self) {
        crate::livenotes::release(self.0, self.1);
    }
}

/// A sign-in, start to end: live jobs on the profile held back meanwhile, let go once its lease is
/// released, then the outcome. One for menu choices opens the browser only while it is still
/// asked for (`wanted`, `still_chosen`).
fn run(p: &Profile, generation: u64) {
    with(|s| {
        if let Some(f) = s.flows.get_mut(&p.id).filter(|f| f.generation == generation) {
            f.phase = Phase::Starting; // a menu's check found it signed out: the sign-in begins
        }
    });
    let outcome = {
        crate::livenotes::defer(&p.id, generation);
        let _deferral = Deferral(&p.id, generation);
        login(p, generation)
    };
    finish(p, generation, outcome);
}

/// The window's Claude sessions on a profile hold its lease while they run: closed for a sign-in.
#[cfg(feature = "tray")]
fn close_sessions(profile: &str) {
    crate::terminal::close_profile(profile);
}

#[cfg(not(feature = "tray"))]
fn close_sessions(_profile: &str) {}

/// Is a Claude session in the window (still) what holds the profile.
#[cfg(feature = "tray")]
fn session_holds(profile: &str) -> bool {
    crate::terminal::holds_profile(profile)
}

#[cfg(not(feature = "tray"))]
fn session_holds(_profile: &str) -> bool {
    false
}

/// The sign-in itself, under the profile's exclusive lease (dropped when this returns — after its
/// child was stopped and reaped). None: cancelled.
fn login(p: &Profile, generation: u64) -> Option<Outcome> {
    let cancelled = || !current(&p.id, generation);
    let t = Instant::now();
    // Live jobs were told to stop (`defer`): seconds each (TERM, KILL after 3 s). Claude sessions in
    // the window on this profile are closed (they resume by id) — only when one of them is what holds
    // it, never for nothing. A minutes run keeps its lease to the end: give up then.
    let mut closed = false;
    let lease = loop {
        match ai::sign_in_lease(p) {
            Ok(Some(l)) => {
                with(|s| s.holding.insert(p.id.clone()));
                break Holding { id: p.id.clone(), lease: Some(l) };
            }
            Err(e) => return Some(Outcome::Failed(format!("{e:#}"))),
            Ok(None) if cancelled() => return None,
            Ok(None) if !closed && session_holds(&p.id) => {
                closed = true;
                close_sessions(&p.id);
            }
            Ok(None) if t.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(200)),
            Ok(None) if session_holds(&p.id) => {
                return Some(Outcome::Failed(format!("{} is held by a Claude session in Echo's window that didn't stop — quit Echo and start it again, then sign in", p.name)));
            }
            Ok(None) => {
                return Some(Outcome::Failed(format!("{} is in use — minutes being written, or a sign-in elsewhere (`echoctl ai login`) — try again when it ends", p.name)));
            }
        }
    };
    // Prepared outside the flows' mutex (its checks may run processes); started under it, once it
    // is clear that nothing stopped or superseded it: not quitting, not cancelled or replaced, and
    // still asked for (Sign in…, or a menu choice that is still the latest of its purpose).
    let mut command = match ai::prepare_claude(p, &ai::login_args(p), &paths::config_dir(), false) {
        Ok(c) => c,
        Err(e) => return Some(Outcome::Failed(format!("{e:#}"))),
    };
    while !still_chosen(&p.id, generation) {
        if !again(&p.id, generation) {
            return None; // no longer asked for
        }
    }
    let spawned = with(|s| {
        let Shared { flows, purposes, shutdown, .. } = s;
        match flows.get_mut(&p.id).filter(|f| f.generation == generation && !f.cancel) {
            Some(f) if !*shutdown && f.wanted(purposes) => {
                f.phase = Phase::Waiting;
                f.since = Instant::now();
                Some(command.spawn())
            }
            Some(f) => {
                f.cancel = true; // no longer asked for: a choice from now on starts a new flow
                None
            }
            None => None,
        }
    });
    let mut child = match spawned {
        None => return None,
        Some(Err(e)) => return Some(Outcome::Failed(format!("couldn't start Claude Code: {e}"))),
        Some(Ok(c)) => c,
    };
    let waited = ai::wait_output(&mut child, DEADLINE, &cancelled);
    if waited.is_err() && !matches!(child.try_wait(), Ok(Some(_))) {
        // `wait_output` stopped and reaped it when it was cancelled or ran out of time; not after
        // an error watching it.
        ai::stop_child(&mut child);
    }
    let outcome = match waited {
        _ if cancelled() => None,
        Err(_) if t.elapsed() >= DEADLINE => Some(Outcome::TimedOut),
        Err(e) => Some(Outcome::Failed(format!("{e:#}"))),
        Ok((out, err, false)) => Some(Outcome::Failed(ai::last_words(&out, &err))),
        // Under our own exclusive lease (`observe` would be refused): no job starts before the
        // account is judged.
        Ok((_, _, true)) => match ai::observe_under(p, lease.lease.as_ref().expect("held until dropped"), &cancelled) {
            _ if cancelled() => None,
            Err(e) => Some(Outcome::Failed(format!("signed in, but the check failed: {e:#}"))),
            Ok(None) => Some(Outcome::Failed("the sign-in didn't finish (still signed out)".into())),
            Ok(Some(a)) => Some(match ai::settle(p, Some(&a)) {
                Ok(_) => Outcome::SignedIn(a.label()),
                Err(e) => match other_account(&e) {
                    Some((now, expected)) => Outcome::OtherAccount { now, expected },
                    None => Outcome::Failed(format!("{e:#}")),
                },
            }),
        },
    };
    drop(lease);
    outcome
}

/// After the lease was released: the outcome shown, said, and acted on — only by the flow that is
/// still current. A cancelled one is cleared (its state is then what the folder says, looked at
/// again — a cancelled re-sign-in may have left the previous account signed in).
fn finish(p: &Profile, generation: u64, outcome: Option<Outcome>) {
    let mine = with(|s| s.flows.get(&p.id).is_some_and(|f| f.generation == generation));
    if !mine {
        return; // replaced by a newer flow: that one speaks
    }
    let Some(outcome) = outcome.filter(|_| current(&p.id, generation)) else {
        clear(&p.id, generation);
        refresh(p, true);
        return;
    };
    if !done(p, generation, outcome.clone()) {
        refresh(p, true); // cancelled at the last moment: what the folder says now
        return;
    }
    if matches!(outcome, Outcome::SignedIn(_)) {
        recovered(&p.id);
    }
    say(p, &outcome);
}

/// The outcome as a notification (the user may be in the browser, or the window closed).
fn say(p: &Profile, outcome: &Outcome) {
    match outcome {
        Outcome::SignedIn(label) => crate::app::notify(&format!("Signed in · {}", p.name), label),
        Outcome::OtherAccount { now, expected } => crate::app::notify(
            &format!("{} is signed in as {now}", p.name),
            &format!("It was set up for {expected} — open Echo to use this account, or sign in again."),
        ),
        Outcome::Failed(why) => crate::app::notify(&format!("The sign-in of {} didn't finish", p.name), why),
        Outcome::TimedOut => crate::app::notify(&format!("The sign-in of {} didn't finish", p.name), "No sign-in within 10 minutes — Sign in… again."),
    }
}

/// Stop the sign-in of `id` (its child is stopped and reaped by its own thread).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn cancel(id: &str) {
    with(|s| {
        if let Some(f) = s.flows.get_mut(id) {
            f.cancel = true;
        }
    });
}

/// Use the account the folder is signed in to now — the one the user was shown. Its check stops
/// when the app quits (and none starts after).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn accept(id: &str, shown: &str) {
    let (id, shown) = (id.to_string(), shown.to_string());
    let admitted = with(|s| {
        if !s.shutdown {
            s.active += 1;
        }
        !s.shutdown
    });
    if !admitted {
        return;
    }
    std::thread::spawn(move || {
        let _active = Active;
        match ai::accept_account(&id, Some(&shown), &|| with(|s| s.shutdown)) {
            Err(_) if with(|s| s.shutdown) => {}
            Ok(label) => {
                // An outcome shown for it no longer holds (a sign-in going on is left alone).
                with(|s| {
                    if s.flows.get(&id).is_some_and(|f| matches!(f.phase, Phase::Done { .. })) {
                        s.flows.remove(&id);
                    }
                });
                recovered(&id);
                crate::app::notify("Account accepted", &label);
            }
            Err(e) => crate::app::notify("Couldn't use that account", &format!("{e:#}")),
        }
    });
}

/// `id` can be used again (signed in, its account accepted, or found usable): live notes waiting
/// for it go on.
pub fn recovered(id: &str) {
    ai::forget_check(id);
    crate::livenotes::signed_in(id);
}

/// Check `p`'s sign-in in the background (at most one at a time; `force` skips the 30 s spacing).
/// Found usable when it wasn't (a sign-in outside the app, say): `recovered`.
pub fn refresh(p: &Profile, force: bool) {
    if !p.kind.signs_in() {
        return;
    }
    let go = with(|s| {
        if s.shutdown {
            return false;
        }
        let busy = s.refreshing.get(&p.id).is_some_and(|(at, running)| *running || (!force && at.elapsed() < REFRESH_EVERY));
        if busy {
            return false;
        }
        s.refreshing.insert(p.id.clone(), (Instant::now(), true));
        s.active += 1;
        true
    });
    if !go {
        return;
    }
    let p = p.clone();
    std::thread::spawn(move || {
        let _active = Active;
        // Usable before — or refused by a model under this token, which no `auth status` undoes.
        let was = ai::refused_by_model(&p) || ai::observation(&p.id).is_some_and(|o| usable(&p, &o));
        // Published by `observe`. Refused while a sign-in holds the lock: nothing was learned, so
        // the next look (once it is released) checks again at once.
        let refused = ai::observe(&p, &|| with(|s| s.shutdown)).is_err_and(|e| e.downcast_ref::<ai::SigningIn>().is_some());
        let now = ai::observation(&p.id).is_some_and(|o| usable(&p, &o));
        with(|s| {
            if refused {
                s.refreshing.remove(&p.id);
            } else if let Some(r) = s.refreshing.get_mut(&p.id) {
                r.1 = false;
            }
        });
        if now && !was && !with(|s| s.shutdown) {
            recovered(&p.id);
        }
    });
}

/// Every Claude profile's sign-in, in the background (the app's start, the window opening).
pub fn refresh_all() {
    if let Ok(s) = ai::Settings::load() {
        for p in s.profiles.iter().filter(|p| p.kind.signs_in()) {
            refresh(p, false);
        }
    }
}

/// What the window and the tray show for `p` (`claude` profiles; None for the others). No process
/// runs here; a background check is started when nothing current is known.
pub fn state(p: &Profile) -> Option<SignInState> {
    if !p.kind.signs_in() {
        return None;
    }
    let flow = with(|s| s.flows.get(&p.id).cloned());
    let obs = ai::observation(&p.id);
    let seq = obs.as_ref().map_or(0, |o| o.seq);
    if let Some(f) = &flow {
        match &f.phase {
            Phase::Starting | Phase::Checking | Phase::Waiting if f.cancel => return Some(SignInState::Stopping),
            Phase::Starting => return Some(SignInState::Starting),
            Phase::Checking => return Some(SignInState::Checking),
            Phase::Waiting => return Some(SignInState::Waiting { email: f.email.clone() }),
            Phase::Done { .. } => {}
        }
    }
    if ai::signing_in_now(p) {
        return Some(SignInState::Elsewhere);
    }
    let token = ai::token_now(p);
    if let Some(f) = &flow {
        match &f.phase {
            Phase::Done { outcome, seen, token: at } if *at == token => {
                let age = f.since.elapsed();
                // A newer observation supersedes the outcome — unless it says the same, signed in.
                let same = obs.as_ref().is_some_and(|o| matches!(&o.result, Ok(Some(a)) if p.expects(a)));
                match outcome {
                    Outcome::SignedIn(label) if age < Duration::from_secs(60) && (seq <= *seen || same) => {
                        return Some(SignInState::SignedIn { account: label.clone(), just: true });
                    }
                    _ if seq > *seen || age >= Duration::from_secs(120) => {}
                    Outcome::SignedIn(_) => {}
                    Outcome::OtherAccount { now, expected } => return Some(SignInState::OtherAccount { account: now.clone(), expected: expected.clone() }),
                    Outcome::Failed(why) => return Some(SignInState::Failed { error: why.clone() }),
                    Outcome::TimedOut => return Some(SignInState::Failed { error: "No sign-in within 10 minutes.".into() }),
                }
            }
            _ => {}
        }
    }
    let Some(o) = obs.filter(|o| !o.stale && token.as_deref() == Some(o.token.as_str())) else {
        refresh(p, false);
        return Some(SignInState::Unknown { error: ai::observation(&p.id).and_then(|o| o.result.err()) });
    };
    Some(match o.result {
        Ok(None) => SignInState::SignedOut,
        Ok(Some(a)) if p.expects(&a) => SignInState::SignedIn { account: a.key(), just: false },
        Ok(Some(a)) => SignInState::OtherAccount { account: a.key(), expected: p.account.clone().unwrap_or_default() },
        Err(e) => SignInState::Unknown { error: Some(e) },
    })
}

/// The app is quitting: every flow and check stops (their children are stopped and reaped by
/// their own threads — waited for up to 5 s); nothing new starts.
pub fn shutdown() {
    with(|s| {
        s.shutdown = true;
        for f in s.flows.values_mut() {
            f.cancel = true;
        }
    });
    let t = Instant::now();
    while with(|s| s.active > 0) && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// (tests) Wait (≤ 15 s) for the background threads to end: a test's checks never outlive its
/// folders (they would read the next test's, or publish into its observations).
#[cfg(test)]
pub fn wait_idle() {
    let t = Instant::now();
    while with(|s| s.active > 0) && t.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::ai::fake;
    use std::fs;

    /// A fresh start — once the threads of the test before are done (a background check it left
    /// would count down this one's threads, or publish into its observations).
    fn reset() {
        until("the previous test's threads", idle);
        *SHARED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn until(what: &str, f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(15), "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The state as the window gets it.
    fn json(p: &Profile) -> serde_json::Value {
        serde_json::to_value(state(p)).unwrap()
    }

    fn st(p: &Profile) -> String {
        json(p)["state"].as_str().unwrap_or("").to_string()
    }

    fn idle() -> bool {
        with(|s| s.active == 0)
    }

    fn gone(pid_file: &std::path::Path) -> bool {
        let pid: i32 = fs::read_to_string(pid_file).unwrap().trim().parse().unwrap();
        unsafe { libc::kill(pid, 0) != 0 }
    }

    #[test]
    fn signing_in_from_the_app() {
        let (_env, root) = crate::paths::testenv::folders("signin-ok");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "after.json"), fake::signed_in("pat@example.com")).unwrap();
        fs::write(fake::file(&p, "login"), "ok").unwrap();
        let call = root.join("home/calls/2026-10-23_121500-signin"); // a stamp no other test uses
        fs::create_dir_all(&call).unwrap();
        fs::write(call.join("meta.json"), r#"{"ai_profile": "w"}"#).unwrap();
        crate::livenotes::test_call_needing(&call, "w");
        start("w");
        until("signed in", || json(&p)["just"] == true);
        assert_eq!(json(&p)["account"], "pat@example.com · example corp (max)");
        until("the call waiting for it goes on", || crate::livenotes::status(&call)["needs_sign_in"].is_null());
        assert_eq!(crate::livenotes::status(&call)["state"], "waiting");
        assert!(fs::read_to_string(fake::file(&p, "login.args")).unwrap().starts_with("auth login --claudeai"));
        assert_eq!(ai::Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some("pat@example.com · example corp"), "first use recorded");
        crate::paths::testenv::eventually("the lease is released", || ai::lease(&p).is_ok());
        until("threads ended", idle);
    }

    #[test]
    fn signing_in_again_replaces_the_one_going_on() {
        let (_env, root) = crate::paths::testenv::folders("signin-again");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        let pid_file = fake::file(&p, "login.pid");
        start("w");
        until("the first browser step", || pid_file.exists());
        let first = root.join("first.pid");
        fs::rename(&pid_file, &first).unwrap();
        start("w"); // the tab was closed: Sign in… again
        until("the second browser step", || pid_file.exists() && st(&p) == "waiting");
        until("the first one stopped", || gone(&first));
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(st(&p), "waiting", "the replaced flow says nothing");
        cancel("w");
        until("stopped", idle);
        assert!(gone(&pid_file));
    }

    #[test]
    fn another_account_is_shown_never_taken_and_can_be_used() {
        let (_env, root) = crate::paths::testenv::folders("signin-other");
        reset();
        let _fake = fake::install(&root);
        let pat = "pat@example.com · example corp";
        let p = fake::profile("w", Some(pat), &fake::signed_in("other@example.com"));
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        // Chosen in a menu: shown, no browser, nothing recorded.
        ensure("w", "record");
        until("checked", idle);
        assert!(!fake::file(&p, "login.args").exists());
        assert_eq!(json(&p), serde_json::json!({ "state": "other_account", "account": "other@example.com · example corp", "expected": pat }));
        assert_eq!(ai::Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some(pat));
        // A sign-in that ends on it: the same.
        fs::write(fake::file(&p, "status.json"), fake::SIGNED_OUT).unwrap();
        fs::write(fake::file(&p, "after.json"), fake::signed_in("other@example.com")).unwrap();
        fs::write(fake::file(&p, "login"), "ok").unwrap();
        start("w");
        until("ended", idle);
        assert_eq!(st(&p), "other_account");
        assert_eq!(ai::Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some(pat));
        // Use it.
        accept("w", "other@example.com · example corp");
        until("accepted", idle);
        assert_eq!(ai::Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some("other@example.com · example corp"));
        until("signed in", || st(&profile("w").unwrap()) == "signed_in");
        // Signed in and fine, seen just now: choosing it again runs no check.
        let runs = fake::status_runs(&p);
        ensure("w", "record");
        until("nothing to do", idle);
        assert_eq!(fake::status_runs(&p), runs);
    }

    #[test]
    fn a_sign_in_elsewhere_is_shown_and_waited_for() {
        let (_env, root) = crate::paths::testenv::folders("signin-elsewhere");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        let cli = ai::sign_in_lease(&p).unwrap().unwrap(); // `echoctl ai login` in a terminal
        assert_eq!(st(&p), "elsewhere");
        let runs = fake::status_runs(&p);
        refresh(&p, true);
        ensure("w", "record");
        until("idle", idle);
        assert_eq!(fake::status_runs(&p), runs, "no check while it holds the lock");
        assert!(!fake::file(&p, "login.args").exists(), "and no second sign-in");
        // It signed in: the window finds out (the token changed).
        fs::write(fake::file(&p, "status.json"), fake::signed_in("pat@example.com")).unwrap();
        drop(cli);
        until("signed in", || st(&p) == "signed_in");
    }

    #[test]
    fn quitting_stops_an_acceptance_check_and_admits_none_after() {
        let (_env, root) = crate::paths::testenv::folders("signin-accept-quit");
        reset();
        let _fake = fake::install(&root);
        let pat = "pat@example.com · example corp";
        let p = fake::profile("w", Some(pat), &fake::signed_in("other@example.com"));
        fs::write(fake::file(&p, "stall"), "").unwrap();
        accept("w", "other@example.com · example corp");
        until("its check runs", || fake::status_runs(&p) == 1);
        let t = Instant::now();
        shutdown();
        assert!(idle() && t.elapsed() < Duration::from_secs(4), "its stalled check was stopped ({:?})", t.elapsed());
        assert_eq!(ai::Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some(pat), "nothing accepted");
        accept("w", "other@example.com · example corp");
        assert!(idle(), "none after quitting");
        reset();
    }

    #[test]
    fn a_choice_changed_before_the_browser_opens_opens_nothing() {
        let (_env, root) = crate::paths::testenv::folders("signin-late-change");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        let job = ai::lease(&p).unwrap(); // a job holds the profile: the sign-in waits for its lock
        let request = choose("record", "w");
        ensure_request("w", "record", request);
        until("waiting for the lock", || st(&p) == "starting" || st(&p) == "checking");
        std::thread::sleep(Duration::from_millis(300));
        choose("record", "system"); // the user picked another profile meanwhile
        drop(job);
        until("ended", idle);
        assert!(!fake::file(&p, "login.args").exists(), "the older choice opened nothing");
        // Back and forth while the first check still runs: the last choice counts.
        fs::write(fake::file(&p, "slow"), "").unwrap();
        let first = choose("record", "w");
        ensure_request("w", "record", first);
        choose("record", "system");
        let again = choose("record", "w");
        ensure_request("w", "record", again);
        until("the latest choice signs in", || fake::file(&p, "login.args").exists());
        cancel("w");
        until("stopped", idle);
        // Chosen in two menus, then changed in one: the other still wants it.
        fs::remove_file(fake::file(&p, "login.args")).unwrap();
        ensure("w", "record");
        ensure("w", "notes:2026-10-23_110000-x"); // joins the check going on
        choose("notes:2026-10-23_110000-x", "system");
        until("Record's choice signs in", || fake::file(&p, "login.args").exists());
        cancel("w");
        until("stopped", idle);
        // Changed and changed back while the first check is already stopping: the last choice
        // starts a check of its own (it doesn't join the one going away).
        fs::remove_file(fake::file(&p, "login.args")).unwrap();
        let _ = fs::remove_file(fake::file(&p, "status.count"));
        fs::write(fake::file(&p, "stall"), "").unwrap();
        fs::write(fake::file(&p, "stubborn"), "").unwrap(); // stopping it takes 3 s
        ensure("w", "record");
        until("its check runs", || fake::status_runs(&p) == 1);
        fs::remove_file(fake::file(&p, "stall")).unwrap(); // (checks from now on are quick)
        fs::remove_file(fake::file(&p, "stubborn")).unwrap();
        choose("record", "system");
        until("it stops", || st(&p) == "stopping");
        ensure("w", "record");
        until("the last choice signs in", || fake::file(&p, "login.args").exists());
        cancel("w");
        until("stopped", idle);
        // Chosen again while a sign-in waits for the lock, its check sent late — it joins right
        // when the sign-in gives up (its first choice changed): it decides again and signs in.
        static LATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        fs::remove_file(fake::file(&p, "login.args")).unwrap();
        let job = ai::lease(&p).unwrap();
        ensure("w", "record");
        until("waiting for the lock", || st(&p) == "starting");
        choose("record", "system");
        LATE.store(choose("record", "w"), std::sync::atomic::Ordering::SeqCst); // registered now, sent later
        *BEFORE_GIVING_UP.lock().unwrap() = Some(|| ensure_request("w", "record", LATE.load(std::sync::atomic::Ordering::SeqCst)));
        drop(job);
        until("the late choice signs in", || fake::file(&p, "login.args").exists());
        *BEFORE_GIVING_UP.lock().unwrap() = None;
        cancel("w");
        until("stopped", idle);
        // The same, its checks started in the opposite order: the older one replaces nothing.
        fs::remove_file(fake::file(&p, "login.args")).unwrap();
        let older = choose("record", "w");
        choose("record", "system");
        let newer = choose("record", "w");
        ensure_request("w", "record", newer);
        ensure_request("w", "record", older);
        until("the latest choice signs in", || fake::file(&p, "login.args").exists());
        cancel("w");
        until("stopped", idle);
    }

    #[test]
    fn a_model_refusal_isnt_undone_by_auth_status_alone() {
        let (_env, root) = crate::paths::testenv::folders("signin-refused");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, &fake::signed_in("pat@example.com"));
        let call = root.join("home/calls/2026-10-23_181500-refused");
        fs::create_dir_all(&call).unwrap();
        fs::write(call.join("meta.json"), r#"{"ai_profile": "w"}"#).unwrap();
        crate::livenotes::test_call_needing(&call, "w");
        ai::auth_failed(&p); // a model said "not logged in"; `auth status` still says signed in
        refresh(&p, true);
        until("checked", idle);
        assert_eq!(st(&p), "signed_in");
        assert_eq!(crate::livenotes::status(&call)["needs_sign_in"], "w", "still needs a sign-in");
        // A check that fails in between doesn't wipe that out either.
        fs::write(fake::file(&p, "status.json"), "not json").unwrap();
        refresh(&p, true);
        until("checked", idle);
        fs::write(fake::file(&p, "status.json"), fake::signed_in("pat@example.com")).unwrap();
        refresh(&p, true);
        until("checked", idle);
        assert_eq!(crate::livenotes::status(&call)["needs_sign_in"], "w", "still needs a sign-in");
        // A sign-in in a terminal since (a new token): that does count.
        drop(ai::sign_in_lease(&p).unwrap().unwrap());
        ai::auth_failed(&p);
        drop(ai::sign_in_lease(&p).unwrap().unwrap());
        refresh(&p, true);
        until("recovered", || crate::livenotes::status(&call)["needs_sign_in"].is_null());
    }

    #[test]
    fn a_finished_outcome_gives_way_to_a_sign_in_elsewhere() {
        let (_env, root) = crate::paths::testenv::folders("signin-outcome-token");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "fail").unwrap();
        start("w");
        until("failed", || st(&p) == "failed");
        let cli = ai::sign_in_lease(&p).unwrap().unwrap(); // then `echoctl ai login` in a terminal
        assert_eq!(st(&p), "elsewhere");
        fs::write(fake::file(&p, "status.json"), fake::signed_in("pat@example.com")).unwrap();
        drop(cli);
        until("its result, not the old failure", || st(&profile("w").unwrap()) == "signed_in");
    }

    #[test]
    fn chosen_again_while_a_cancelled_sign_in_stops() {
        let (_env, root) = crate::paths::testenv::folders("signin-cancel-rechoose");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        fs::write(fake::file(&p, "stubborn"), "").unwrap(); // stopping its login takes 3 s
        let (args, pid) = (fake::file(&p, "login.args"), fake::file(&p, "login.pid"));
        start("w");
        until("the browser step", || pid.exists() && st(&p) == "waiting");
        let first = root.join("first.pid");
        fs::rename(&pid, &first).unwrap();
        fs::remove_file(&args).unwrap();
        cancel("w");
        fs::remove_file(fake::file(&p, "stubborn")).unwrap(); // (the next one stops at once)
        until("stopping", || st(&p) == "stopping");
        ensure("w", "record"); // chosen in a menu while the cancelled one still holds the lock
        until("the choice signs in once the lock is free", || args.exists() && pid.exists());
        assert!(gone(&first));
        cancel("w");
        until("stopped", idle);
        assert!(gone(&pid));
    }

    #[test]
    fn a_lock_let_go_while_a_check_was_refused_is_looked_at_again() {
        static HELD: Mutex<Option<Holding>> = Mutex::new(None);
        let (_env, root) = crate::paths::testenv::folders("signin-let-go");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        // An in-app sign-in that is ending holds the lock …
        let lease = ai::sign_in_lease(&p).unwrap().unwrap();
        with(|s| s.holding.insert("w".into()));
        *HELD.lock().unwrap() = Some(Holding { id: "w".into(), lease: Some(lease) });
        // … and lets go right after a check was refused by it, before the check asks whose it was.
        *AFTER_REFUSED.lock().unwrap() = Some(|| drop(HELD.lock().unwrap().take()));
        ensure("w", "record");
        until("the choice signs in", || fake::file(&p, "login.args").exists());
        *AFTER_REFUSED.lock().unwrap() = None;
        cancel("w");
        until("stopped", idle);
    }

    #[test]
    fn cancelled_at_the_last_moment_isnt_left_stopping() {
        let (_env, root) = crate::paths::testenv::folders("signin-late-cancel");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "fail").unwrap();
        *BEFORE_DONE.lock().unwrap() = Some(|| cancel("w"));
        start("w");
        until("ended", idle);
        *BEFORE_DONE.lock().unwrap() = None;
        assert!(with(|s| s.flows.is_empty()), "cleared, not left cancelled");
        until("the folder's own state", || st(&p) == "signed_out");
    }

    #[test]
    fn a_failed_sign_in_says_why() {
        let (_env, root) = crate::paths::testenv::folders("signin-fail");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "fail").unwrap();
        start("w");
        until("failed", || st(&p) == "failed");
        assert!(json(&p)["error"].as_str().unwrap().contains("Login failed: Request failed with status code 400"));
        until("threads ended", idle);
    }

    #[test]
    fn cancelling_stops_the_login_and_quitting_starts_nothing_after() {
        let (_env, root) = crate::paths::testenv::folders("signin-cancel");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        start("w");
        until("the browser step", || fake::file(&p, "login.pid").exists() && st(&p) == "waiting");
        assert!(ai::lease(&p).is_err(), "jobs wait meanwhile");
        cancel("w");
        until("stopped", idle);
        assert!(gone(&fake::file(&p, "login.pid")), "the login child is gone");
        until("the folder's own state again", || st(&p) == "signed_out");
        crate::paths::testenv::eventually("the lease is released", || ai::lease(&p).is_ok());
        // Quitting during a sign-in: its child is stopped, and nothing starts afterwards.
        fs::remove_file(fake::file(&p, "login.pid")).unwrap();
        start("w");
        until("the browser step", || fake::file(&p, "login.pid").exists());
        shutdown();
        assert!(idle());
        assert!(gone(&fake::file(&p, "login.pid")));
        start("w");
        ensure("w", "record");
        assert!(with(|s| s.flows.is_empty()), "nothing after shutdown");
        reset();
    }

    #[test]
    fn choosing_a_profile_signs_in_only_when_plainly_signed_out() {
        let (_env, root) = crate::paths::testenv::folders("signin-ensure");
        reset();
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        fs::write(fake::file(&p, "login"), "hang").unwrap();
        let args = fake::file(&p, "login.args");
        // Signed out: the sign-in starts.
        ensure("w", "record");
        until("the login", || args.exists());
        cancel("w");
        until("stopped", idle);
        // An answer that isn't an explicit "signed out": no browser.
        fs::remove_file(&args).unwrap();
        fs::write(fake::file(&p, "status.json"), r#"{"error": "keychain locked"}"#).unwrap();
        ensure("w", "record");
        until("checked", idle);
        assert!(!args.exists());
        assert_eq!(st(&p), "unknown");
        // Signed out, but another profile was chosen while it was checked: no browser.
        fs::write(fake::file(&p, "status.json"), fake::SIGNED_OUT).unwrap();
        fs::write(fake::file(&p, "slow"), "").unwrap();
        ensure("w", "record");
        choose("record", "system");
        until("checked", idle);
        assert!(!args.exists(), "the older choice opened nothing");
        // A call: only once the switch was applied, and while the call still uses it.
        fs::remove_file(fake::file(&p, "slow")).unwrap();
        let call = root.join("home/calls/2026-10-23_110000-sync");
        fs::create_dir_all(&call).unwrap();
        fs::write(call.join("meta.json"), r#"{"ai_profile": "system"}"#).unwrap();
        choose("call:2026-10-23_110000-sync", "w");
        applied("2026-10-23_110000-sync", "w");
        until("checked", idle);
        assert!(!args.exists(), "the call doesn't use it (the switch didn't apply)");
        fs::write(call.join("meta.json"), r#"{"ai_profile": "w"}"#).unwrap();
        applied("2026-10-23_110000-sync", "w");
        until("the login", || args.exists());
        cancel("w");
        until("stopped", idle);
        // Retitled while it is checked: still that call, found by its stamp.
        fs::remove_file(&args).unwrap();
        fs::write(fake::file(&p, "slow"), "").unwrap();
        choose("call:2026-10-23_110000-sync", "w");
        applied("2026-10-23_110000-sync", "w");
        fs::rename(&call, root.join("home/calls/2026-10-23_110000-retitled")).unwrap();
        until("the login", || args.exists());
        cancel("w");
        until("stopped", idle);
        reset();
    }
}
