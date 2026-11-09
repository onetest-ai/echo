//! Claude Code for Echo's short model jobs (live notes, questions, the Brain's text and page jobs): a lean run with no
//! tools, our system prompt and the text on stdin. For a call's live-notes summaries, one conversation is kept open
//! for the whole recording: a single process answers turn after turn over stream-json. Measured (M4 Max): starting
//! Claude Code costs about 4.7 s of CPU and 400 MB; a turn in a kept process about 0.3 s, its history read from the
//! prompt cache (#239).

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::ai::{self, Profile};
use crate::livenotes::{clip, last_line, set_usage};
use crate::paths;

pub(crate) fn lean_settings() -> Result<PathBuf> {
    let f = paths::config_dir().join("lean-job-settings.json");
    let want = br#"{"claudeMdExcludes": ["**"]}"#;
    if fs::read(&f).ok().as_deref() != Some(&want[..]) {
        fs::create_dir_all(paths::config_dir())?;
        paths::write_atomic(&f, want)?;
    }
    Ok(f)
}

/// Claude Code's arguments for a lean job: no tools, no MCP, nothing kept on disk; `stream`: stream-json in and out.
fn args(model: &str, system: &str, stream: bool) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec!["-p".into()];
    if stream {
        args.extend(["--input-format", "stream-json", "--output-format", "stream-json", "--verbose"].map(String::from));
    } else {
        args.extend(["--output-format", "json"].map(String::from));
    }
    args.extend([
        "--system-prompt".into(),
        system.into(),
        "--tools".into(),
        String::new(),
        "--strict-mcp-config".into(),
        "--no-session-persistence".into(),
        "--model".into(),
        model.to_string(),
        "--settings".into(),
        lean_settings()?.display().to_string(),
    ]);
    Ok(args)
}

/// Claude Code for `p` with `args`, where a job of its kind runs: `system` where the minutes run (the data folder or
/// `notes_in`: its `.claude/settings.json` is part of "as configured"), the others in Echo's state folder.
fn command(p: &Profile, args: &[String]) -> Result<std::process::Command> {
    let cwd = if p.kind.managed() { paths::state_dir() } else { ai::system_folder()? };
    fs::create_dir_all(&cwd)?;
    let mut cmd = ai::prepare_claude(p, args, &cwd, true)?;
    cmd.env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1");
    Ok(cmd)
}

/// The model's text from a run's result envelope, or why there is none: not signed in, an error, cut off, empty.
fn reply(p: &Profile, result: Option<&Value>, out: &str, err: &str, wrote: Option<String>, whole: bool) -> Result<String> {
    let text = result.and_then(|v| v["result"].as_str()).unwrap_or("").to_string();
    let is_error = result.is_none_or(|v| v["is_error"].as_bool().unwrap_or(false));
    if is_error {
        let all = format!("{out}\n{err}\n{text}");
        if crate::notes::needs_login(&all) {
            if !p.kind.signs_in() {
                bail!("Claude Code isn't signed in — open Terminal, run claude and type /login");
            }
            ai::auth_failed(p);
            return Err(anyhow::Error::new(ai::SignInNeeded::signed_out(p)));
        }
        let why = [text.trim().lines().next().unwrap_or("").to_string(), last_line(err)].into_iter().find(|l| !l.is_empty());
        let why = why.or(wrote);
        bail!("{}: {}", p.name, clip(&why.unwrap_or_else(|| "Claude Code failed".into()), 200));
    }
    if whole && result.is_some_and(|v| v["stop_reason"] == "max_tokens") {
        bail!("{}'s reply was cut off at the token limit", p.name);
    }
    if text.trim().is_empty() {
        bail!("{} returned no text", p.name);
    }
    Ok(text)
}

/// One lean run; with `blocks` (Messages API content: images, text) the prompt goes in as one stream-json user message
/// instead of plain text.
#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_claude_with(p: &Profile, model: &str, system: &str, user: &str, blocks: Option<&Value>, whole: bool, deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<String> {
    let input = match blocks {
        Some(b) => format!("{}\n", json!({"type": "user", "message": {"role": "user", "content": b}})),
        None => user.to_string(),
    };
    let mut child = command(p, &args(model, system, blocks.is_some())?)?.spawn()?;
    let (out, err, _, wrote) = ai::wait_output_with(&mut child, Some(input.into_bytes()), deadline, cancelled).map_err(|e| anyhow!("{}: {e:#}", p.name))?;
    let result = out.lines().rev().filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok()).find(|v| v["type"] == "result");
    if let Some(v) = &result {
        set_usage(v, None);
    }
    reply(p, result.as_ref(), &out, &err, wrote.err().map(|e| format!("couldn't send the prompt: {e}")), whole)
}

// ── a call's live-notes conversation ───────────────────────────────────────────────────────────

/// After this many turns a conversation starts afresh (its history only grows).
const MAX_TURNS: u32 = 30;

/// One Claude Code process, kept for a call's summaries. Stopped (with its process group) when dropped.
struct Session {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    err: Arc<Mutex<String>>,
    /// Profile and model it runs on: another one starts another conversation.
    key: String,
    /// How far into the live transcript its last answer went.
    cursor: usize,
    turns: u32,
    /// Claude Code reports the cost of the whole conversation so far: what was already counted.
    cost: f64,
}

impl Drop for Session {
    fn drop(&mut self) {
        ai::stop_child(&mut self.child);
    }
}

static SESSIONS: Mutex<Option<HashMap<String, Session>>> = Mutex::new(None);

fn sessions<T>(f: impl FnOnce(&mut HashMap<String, Session>) -> T) -> T {
    f(SESSIONS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new))
}

/// End a call's conversation (`Some(stamp)`), or every one (`None`): the recording ended, the profile changed, Echo quits.
pub(crate) fn forget(stamp: Option<&str>) {
    let gone: Vec<Session> = sessions(|m| match stamp {
        Some(s) => m.remove(s).into_iter().collect(),
        None => m.drain().map(|(_, v)| v).collect(),
    });
    drop(gone); // stopped outside the lock
}

impl Session {
    fn start(p: &Profile, system: &str, key: String) -> Result<Session> {
        let mut child = command(p, &args(&p.live_model(), system, true)?)?.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("{}: no stdin", p.name))?;
        let (tx, lines) = std::sync::mpsc::channel();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        let err = Arc::new(Mutex::new(String::new()));
        if let Some(mut e) = child.stderr.take() {
            let err = err.clone();
            std::thread::spawn(move || {
                let mut b = String::new();
                let _ = e.read_to_string(&mut b);
                err.lock().unwrap_or_else(|e| e.into_inner()).push_str(&b);
            });
        }
        Ok(Session { child, stdin, lines, err, key, cursor: 0, turns: 0, cost: 0.0 })
    }

    /// One turn: `user` in, the model's text out (checked as a one-off run's), within `deadline`.
    fn ask(&mut self, p: &Profile, user: &str, deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<String> {
        let msg = json!({"type": "user", "message": {"role": "user", "content": user}});
        let wrote = writeln!(self.stdin, "{msg}").and_then(|_| self.stdin.flush()).err().map(|e| format!("couldn't send the prompt: {e}"));
        let t = Instant::now();
        let mut out = String::new();
        let result = loop {
            if wrote.is_some() || cancelled() {
                break None;
            }
            if t.elapsed() > deadline {
                bail!("{}: no answer within {} s", p.name, deadline.as_secs());
            }
            match self.lines.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => match serde_json::from_str::<Value>(line.trim()) {
                    Ok(v) if v["type"] == "result" => break Some(v),
                    _ => out.push_str(&line),
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break None, // Claude Code ended
            }
        };
        if cancelled() {
            bail!("stopped");
        }
        if let Some(v) = &result {
            let total = v["total_cost_usd"].as_f64().unwrap_or(self.cost);
            set_usage(v, Some((total - self.cost).max(0.0)));
            self.cost = total;
        }
        let err = self.err.lock().unwrap_or_else(|e| e.into_inner()).clone();
        reply(p, result.as_ref(), &out, &err, wrote, true)
    }
}

/// A live-notes summary of call `stamp` on `p` (Claude Code). The call's kept conversation gets only `new` (the
/// transcript lines since its last answer) when that answer went up to `span.0`; otherwise a new conversation starts
/// with `full`. It then covers up to `span.1`. A failure ends the conversation: the next summary starts a new one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn summary(stamp: &str, p: &Profile, system: &str, full: &str, new: &str, span: (usize, usize), deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<String> {
    let key = format!("{}|{}", p.id, p.live_model());
    let kept = sessions(|m| m.remove(stamp)).filter(|s| s.key == key && s.cursor == span.0 && s.turns < MAX_TURNS);
    let (mut s, user) = match kept {
        Some(s) => (s, format!("{new}\nUpdate the notes with these lines; answer as before.")),
        None => (Session::start(p, system, key)?, full.to_string()),
    };
    let text = s.ask(p, &user, deadline, cancelled)?;
    s.cursor = span.1;
    s.turns += 1;
    let replaced = sessions(|m| m.insert(stamp.to_string(), s));
    drop(replaced);
    Ok(text)
}
