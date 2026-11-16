//! AI profiles: sign-in — leases, what was observed, a run's output (split out of `ai.rs`, moved as it was).

use super::*;

/// The shared lease (a job, a check); a `SigningIn` error while a sign-in holds the lock. Tried
/// for 300 ms: a lock released a moment ago can still look held while another thread here starts a
/// process (its child holds a copy of the descriptor until it execs — measured on macOS).
pub fn lease(p: &Profile) -> Result<Lease> {
    let t = Instant::now();
    let lock = loop {
        match open_lock(p, true)? {
            Some(l) => break l,
            None if t.elapsed() < Duration::from_millis(300) => std::thread::sleep(Duration::from_millis(30)),
            None => return Err(anyhow::Error::new(SigningIn { profile: p.id.clone(), name: p.name.clone() })),
        }
    };
    let token = lock.text().with_context(|| format!("reading {}", p.lock_path().display()))?;
    Ok(Lease { _lock: lock, token })
}

/// A sign-in's exclusive lease, under a new token (None: a job, a check or another sign-in holds
/// the lock).
pub fn sign_in_lease(p: &Profile) -> Result<Option<Lease>> {
    let Some(lock) = open_lock(p, false)? else { return Ok(None) };
    let token = format!("{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default());
    lock.set_text(&token).with_context(|| format!("writing {}", p.lock_path().display()))?;
    forget_check(&p.id);
    Ok(Some(Lease { _lock: lock, token }))
}

/// The token now, without the lock (`""` before the first sign-in); None when the file can't be
/// read — on Windows, while a sign-in holds it.
pub fn token_now(p: &Profile) -> Option<String> {
    match fs::read_to_string(p.lock_path()) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(String::new()),
        Err(_) => None,
    }
}

/// A sign-in holds the profile's lock (probed with a shared take, released at once; a sign-in
/// starting at that very moment tries again).
pub fn signing_in_now(p: &Profile) -> bool {
    matches!(open_lock(p, true), Ok(None))
}

/// What this process last learned of a profile's sign-in (a `claude auth status` check, or
/// `auth_failed`): `seq` taken when the check started (a check finishing late never replaces a
/// newer one) and the token it ran under.
#[derive(Clone, Debug)]
pub struct Observation {
    pub seq: u64,
    pub token: String,
    pub at: Instant,
    pub result: Result<Option<Account>, String>,
    /// Published by `auth_failed` (a model said the folder isn't signed in; `result` is then an
    /// error), not by a check: it needs another look.
    pub stale: bool,
}

pub(super) static LAST: Mutex<Option<HashMap<String, Observation>>> = Mutex::new(None);
pub(super) static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(super) fn next_seq() -> u64 {
    SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

pub(super) fn publish(id: &str, o: Observation) {
    let mut g = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(HashMap::new);
    if m.get(id).is_none_or(|old| old.seq < o.seq) {
        m.insert(id.to_string(), o);
    }
}

/// The last observation of a profile's sign-in (this process).
pub fn observation(id: &str) -> Option<Observation> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(id).cloned())
}

/// Who `p`'s folder is signed in to (None: signed out), checked under a shared lease and
/// published; a `SigningIn` error while a sign-in holds the lock (nothing published: it is a lock
/// state, not an answer). A lock that can't be opened at all is published as the check's error.
pub fn observe(p: &Profile, cancelled: &dyn Fn() -> bool) -> Result<Option<Account>> {
    let seq = next_seq();
    let lease = match lease(p) {
        Ok(l) => l,
        Err(e) if e.downcast_ref::<SigningIn>().is_some() => return Err(e),
        Err(e) => {
            let token = token_now(p).unwrap_or_default();
            publish(&p.id, Observation { seq, token, at: Instant::now(), result: Err(format!("{e:#}")), stale: false });
            return Err(e);
        }
    };
    observe_under(p, &lease, cancelled)
}

/// The check, by a holder of the lock: a sign-in under its exclusive lease (`observe` would be
/// refused — a shared take fails against it, in this process too), a job under its shared one
/// (`check`), so the account can't change before it is used. Published under the lease's token; a
/// stopped check publishes nothing.
pub fn observe_under(p: &Profile, lease: &Lease, cancelled: &dyn Fn() -> bool) -> Result<Option<Account>> {
    let seq = next_seq();
    let r = account_with(p, cancelled);
    if !cancelled() {
        let result = r.as_ref().map(Clone::clone).map_err(|e| format!("{e:#}"));
        publish(&p.id, Observation { seq, token: lease.token.clone(), at: Instant::now(), result, stale: false });
    }
    r
}

/// `claude auth status` for `p`'s folder (stopped when `cancelled()` says so).
pub(super) fn account_with(p: &Profile, cancelled: &dyn Fn() -> bool) -> Result<Option<Account>> {
    let mut child = claude_command(p, &["auth".into(), "status".into()], &paths::config_dir(), false)?;
    let (out, err, _) = wait_output(&mut child, Duration::from_secs(20), cancelled)
        .with_context(|| format!("checking the sign-in of {}", p.name))?;
    parse_status(&out, &err).with_context(|| format!("checking the sign-in of {}", p.name))
}

/// Profiles a model refused (id → the sign-in token then): `auth status` saying "signed in"
/// doesn't undo that under the same token — a sign-in (a new token), an accepted account or a job
/// that works does.
pub(super) static REFUSED: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// A model run said `p` isn't signed in (or refused its token): neither the cached check nor the
/// last observation counts any more, and the refusal holds until a sign-in, an accepted account,
/// or a job that works. The job's own failure (`needs_sign_in`, a `needs_login` run) stays as long.
pub fn auth_failed(p: &Profile) {
    forget_check(&p.id);
    let token = token_now(p).unwrap_or_default();
    REFUSED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(p.id.clone(), token.clone());
    let result = Err("Claude Code said it isn't signed in".to_string());
    publish(&p.id, Observation { seq: next_seq(), token, at: Instant::now(), result, stale: true });
}

/// A model refused `p` under the sign-in token it has now (`auth_failed`).
pub fn refused_by_model(p: &Profile) -> bool {
    let token = token_now(p);
    REFUSED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&p.id)).is_some_and(|t| token.as_deref() == Some(t.as_str()))
}

/// A job on profile `id` worked (or its account was accepted): a model's refusal no longer holds.
pub fn job_worked(id: &str) {
    if let Some(m) = REFUSED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(id);
    }
}

/// `claude auth status`'s answer: signed in (with an email), or explicitly signed out
/// (`loggedIn: false`); anything else is an error, never "signed out" (that would open a sign-in).
pub(super) fn parse_status(out: &str, err: &str) -> Result<Option<Account>> {
    // The JSON ends the output; the login shell's start-up may print a `{` of its own before it.
    let v: Value = out
        .match_indices('{')
        .find_map(|(i, _)| serde_json::from_str(out[i..].trim()).ok())
        .ok_or_else(|| anyhow!("unexpected answer from `claude auth status`: {}", last_words(out, err)))?;
    match v["loggedIn"].as_bool() {
        Some(false) => return Ok(None),
        Some(true) => {}
        None => bail!("unexpected answer from `claude auth status`: {}", clip_line(&v.to_string())),
    }
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let a = Account { email: s("email"), org: s("orgName"), subscription: s("subscriptionType") };
    if a.email.is_empty() {
        bail!("`claude auth status` names no account");
    }
    Ok(Some(a))
}

/// A failed child's reason: the last line it wrote on stderr, else on stdout (not the sign-in's
/// own chatter: its URLs and prompts).
pub fn last_words(out: &str, err: &str) -> String {
    let said = |s: &str| {
        s.lines().map(str::trim).rfind(|l| !l.is_empty() && !l.contains("://") && !l.starts_with("Opening browser") && !l.starts_with("Paste code")).map(str::to_owned)
    };
    clip_line(&said(err).or_else(|| said(out)).unwrap_or_else(|| "no output".into()))
}

pub(super) fn clip_line(s: &str) -> String {
    let s = s.lines().next().unwrap_or("");
    if s.chars().count() > 160 { format!("{}…", s.chars().take(160).collect::<String>()) } else { s.to_string() }
}

/// Run a child to the end: (stdout, stderr, exited successfully). Stopped (TERM, KILL after 3 s,
/// reaped) at the deadline or as soon as `cancelled()` says so; both are errors.
pub fn wait_output(child: &mut Child, deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<(String, String, bool)> {
    let (out, err, ok, _) = wait_output_with(child, None, deadline, cancelled)?;
    Ok((out, err, ok))
}

/// `wait_output`, also handing `input` to the child's stdin (on a thread of its own: a child that
/// never reads it is supervised all the same); the last value says whether the input got through.
pub fn wait_output_with(child: &mut Child, input: Option<Vec<u8>>, deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<(String, String, bool, std::io::Result<()>)> {
    use std::io::{Read, Write};
    let stdin = child.stdin.take();
    let writer = std::thread::spawn(move || match (stdin, input) {
        (Some(mut i), Some(input)) => i.write_all(&input), // dropped after: closed, the input is complete
        _ => Ok(()),
    });
    fn drain(r: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            String::from_utf8_lossy(&b).into_owned()
        })
    }
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let t = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if cancelled() {
            stop_child(child);
            bail!("stopped");
        }
        if t.elapsed() > deadline {
            stop_child(child);
            bail!("no answer within {} s", deadline.as_secs());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // The leader is gone; something it started may still hold the pipes (a background process
    // from shell start-up): its stdout / stderr, or its stdin with the input half written. Give
    // them 2 s to end — still under the deadline and stopping — then stop what is left of the group,
    // which closes them.
    let exited = Instant::now();
    while !(out.is_finished() && err.is_finished() && writer.is_finished()) {
        let give_up = cancelled() || t.elapsed() > deadline || exited.elapsed() > Duration::from_secs(2);
        if give_up {
            stop_group(child.id());
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let wrote = writer.join().unwrap_or(Ok(()));
    Ok((out.join().unwrap_or_default(), err.join().unwrap_or_default(), status.success(), wrote))
}

/// Why a readiness check failed, kept typed so a cache hit gives the same error as the check.
#[derive(Clone, Debug)]
pub(super) enum Problem {
    SignIn(SignInNeeded),
    Other(String),
}

impl Problem {
    pub(super) fn of(e: &anyhow::Error) -> Problem {
        match e.downcast_ref::<SignInNeeded>() {
            Some(s) => Problem::SignIn(s.clone()),
            None => Problem::Other(format!("{e:#}")),
        }
    }

    pub(super) fn error(&self) -> anyhow::Error {
        match self {
            Problem::SignIn(s) => anyhow::Error::new(s.clone()),
            Problem::Other(s) => anyhow!("{s}"),
        }
    }
}

/// Recent sign-in checks (`claude auth status` takes about a second), per profile id: the token
/// they ran under, when, and the account (or why it can't be used). A success is kept 30 s, a
/// failure 10 s — and neither once a sign-in changed the token.
pub(super) type Checks = HashMap<String, (String, Instant, Result<String, Problem>)>;
pub(super) static CHECKED: Mutex<Option<Checks>> = Mutex::new(None);
