//! AI profiles: a profile checked and settled, and logging in (split out of `ai.rs`, moved as it was).

use super::*;

/// Can a job run on `p` now? `local`: an http(s) server and a model. `claude` (checked under the
/// caller's `lease`): signed in, and to the account the profile was first used with (recorded then).
pub(super) fn check(p: &Profile, lease: Option<&Lease>) -> Result<()> {
    if p.kind == Kind::Local {
        let url = p.base_url.as_deref().unwrap_or("");
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("the AI profile \"{}\" has no http(s) server address", p.name);
        }
        if p.live_model.is_none() && p.notes_model.is_none() {
            bail!("the AI profile \"{}\" names no model", p.name);
        }
    }
    if !p.kind.signs_in() {
        return Ok(());
    }
    let Some(lease) = lease else { bail!("{} is checked without its lock", p.name) };
    let cached = CHECKED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(&p.id).cloned();
    if let Some((token, at, r)) = cached {
        let ttl = Duration::from_secs(if r.is_ok() { 30 } else { 10 });
        if token == lease.token && at.elapsed() < ttl {
            return r.map(|_| ()).map_err(|e| e.error());
        }
    }
    let r = observe_under(p, lease, &|| false).and_then(|a| settle(p, a.as_ref())).map_err(|e| Problem::of(&e));
    CHECKED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(p.id.clone(), (lease.token.clone(), Instant::now(), r.clone()));
    r.map(|_| ()).map_err(|e| e.error())
}

/// Forget the last sign-in check (a sign-in started, an account accepted, a model said it isn't
/// signed in).
pub fn forget_check(id: &str) {
    if let Some(m) = CHECKED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(id);
    }
}

/// What a sign-in check says (`None` = signed out): the account's key when it may be used, and
/// whether it should be recorded as the profile's (its first use); else `SignInNeeded`.
pub(super) fn judge(p: &Profile, a: Option<&Account>) -> Result<(String, bool)> {
    let Some(a) = a else { return Err(anyhow::Error::new(SignInNeeded::signed_out(p))) };
    match &p.account {
        Some(want) if !p.expects(a) => Err(anyhow::Error::new(SignInNeeded::other_account(p, a.key(), want.clone()))),
        Some(_) => Ok((a.key(), false)),
        None => Ok((a.key(), true)),
    }
}

/// Judge who `p`'s folder is signed in to against the profile as stored now (re-read: another
/// process may have recorded or accepted an account meanwhile): the account's key, or
/// `SignInNeeded`. The first use records it (unless another writer recorded a different one).
pub fn settle(p: &Profile, a: Option<&Account>) -> Result<String> {
    let now = Settings::load()?.get(&p.id)?;
    let (key, first) = judge(&now, a)?;
    if first {
        Settings::update(|s| {
            let stored = s.profiles.iter_mut().find(|x| x.id == p.id).ok_or_else(|| anyhow!("the AI profile \"{}\" was removed", p.id))?;
            match stored.account.clone() {
                None => stored.account = Some(key.clone()),
                Some(want) if want.to_lowercase() == key => {}
                Some(want) => return Err(anyhow::Error::new(SignInNeeded::other_account(stored, key.clone(), want))),
            }
            Ok(())
        })?;
    }
    Ok(key)
}

/// Accept the account a `claude` profile's folder is signed in to now — the one the user was shown
/// (`shown`), when given: another one meanwhile is refused. Checked and recorded under the
/// profile's lease (no sign-in in between); the check stops when `cancelled()` says so (its child
/// reaped before the lease goes).
pub fn accept_account(id: &str, shown: Option<&str>, cancelled: &dyn Fn() -> bool) -> Result<String> {
    let p = Settings::load()?.get(id)?;
    if !p.kind.signs_in() {
        bail!("{} has no account of its own to accept", p.name);
    }
    let lease = lease(&p)?;
    let a = observe_under(&p, &lease, cancelled)?.ok_or_else(|| anyhow::Error::new(SignInNeeded::signed_out(&p)))?;
    if cancelled() {
        bail!("stopped");
    }
    if let Some(shown) = shown.filter(|s| s.to_lowercase() != a.key()) {
        bail!("{} is now signed in as {}, not {shown} — look again", p.name, a.key());
    }
    Settings::update(|s| {
        let stored = s.profiles.iter_mut().find(|x| x.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        stored.account = Some(a.key());
        Ok(())
    })?;
    forget_check(id);
    job_worked(id);
    drop(lease);
    Ok(a.label())
}

/// The email pre-filled on the sign-in page: the profile's, else the recorded account's.
pub fn login_email(p: &Profile) -> Option<String> {
    let recorded = p.account.as_deref().and_then(|k| k.split(" · ").next()).map(str::to_owned);
    p.email.clone().or(recorded).filter(|e| e.contains('@'))
}

/// `claude auth login` for `p`: a claude.ai account always (a Console sign-in may be stored outside
/// the profile's folder), the email pre-filled (`login_email`), single sign-on when the profile
/// says so.
pub fn login_args(p: &Profile) -> Vec<String> {
    let mut a: Vec<String> = ["auth", "login", "--claudeai"].map(String::from).to_vec();
    if let Some(email) = login_email(p) {
        a.extend(["--email".into(), email]);
    }
    if p.sso {
        a.push("--sso".into());
    }
    a
}

/// `echoctl ai login <id>`: sign in here, in the terminal it was run from (Claude Code's own
/// prompts, its paste-code fallback included). Ctrl-C stops it.
pub fn login_here(id: &str) -> Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let p = Settings::load()?.get(id)?;
    let claude = fake_claude().or_else(crate::app::claude_bin).ok_or_else(|| anyhow!("Claude Code is not installed — get it at https://claude.com/claude-code"))?;
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let _ = ctrlc::set_handler(move || s.store(true, Ordering::SeqCst));
    let label = login_with(&p, &claude, true, &|| stop.load(Ordering::SeqCst))?;
    println!("{} is signed in as {label}", p.name);
    Ok(())
}

/// The CLI's sign-in, under the profile's exclusive lease (taken within 10 s, as the app does),
/// the child in this terminal's foreground (`terminal`: inherited stdio). One way out for every
/// child: stopped and reaped before the lease is released. The account's label, or why not.
pub(super) fn login_with(p: &Profile, claude: &Path, terminal: bool, stopped: &dyn Fn() -> bool) -> Result<String> {
    if !p.kind.signs_in() {
        bail!("only a Claude account profile signs in");
    }
    if let Some(why) = policy_conflict(p) {
        bail!("{why}");
    }
    let t = Instant::now();
    let lease = loop {
        match sign_in_lease(p)? {
            Some(l) => break l,
            None if stopped() => bail!("stopped — {} isn't signed in", p.name),
            None if t.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(200)),
            None => bail!("{} is being signed in already, or Echo is using it (writing minutes, or a Claude session open in Echo's window on it — close that session) — try again in a moment", p.name),
        }
    };
    fs::create_dir_all(p.config_folder())?;
    // In this terminal's foreground, not `detach`ed: it reads the paste code, and Ctrl-C reaches
    // it. No login shell (its job control would take the terminal): the terminal's environment is
    // already the user's; the profile's routing is applied on top.
    let mut c = build_command(p, claude, &login_args(p), &paths::config_dir(), false, false);
    if terminal {
        c.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        c.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let mut child = c.spawn()?;
    let watched = (|| -> Result<std::process::ExitStatus> {
        loop {
            if let Some(s) = child.try_wait()? {
                return Ok(s);
            }
            if stopped() {
                bail!("stopped");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    })();
    let status = match watched {
        Ok(s) => s,
        Err(e) => {
            stop_foreground(&mut child);
            if !stopped() {
                return Err(e);
            }
            child.wait()?
        }
    };
    if stopped() {
        bail!("stopped — {} isn't signed in", p.name);
    }
    if !status.success() {
        bail!("the sign-in didn't finish");
    }
    let a = match observe_under(p, &lease, stopped) {
        Err(_) if stopped() => bail!("stopped — the sign-in of {} wasn't checked", p.name),
        r => r?,
    };
    let settled = settle(p, a.as_ref());
    drop(lease);
    match settled {
        Ok(_) => Ok(a.map(|a| a.label()).unwrap_or_default()),
        Err(e) if e.downcast_ref::<SignInNeeded>().is_some_and(|s| s.now.is_some()) => Err(anyhow!("{e:#} (or: echoctl ai accept {})", p.id)),
        Err(e) => Err(e),
    }
}

/// Stop a child in our own process group (a terminal's foreground: the group is the terminal's,
/// so only its pid is signalled): TERM, then KILL after 3 s. On Windows its whole tree at once
/// (`taskkill /T /F`: a `claude.cmd` launcher runs Claude Code as its child).
pub(super) fn stop_foreground(child: &mut Child) {
    #[cfg(windows)]
    stop_group(child.id());
    #[cfg(unix)]
    {
        // SAFETY: signalling our own child by pid.
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(3) {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}
