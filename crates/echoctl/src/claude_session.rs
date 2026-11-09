//! Claude Code for Echo's short model jobs (live questions, the Brain's text and page jobs): a lean run with no tools,
//! our system prompt and the text on stdin, its result envelope checked (`reply`) before the model's text is used.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

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
pub(crate) fn command(p: &Profile, args: &[String]) -> Result<std::process::Command> {
    let cwd = if p.kind.managed() { paths::state_dir() } else { ai::system_folder()? };
    fs::create_dir_all(&cwd)?;
    let mut cmd = ai::prepare_claude(p, args, &cwd, true)?;
    cmd.env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1");
    Ok(cmd)
}

/// The model's text from a run's result envelope, or why there is none: not signed in, an error, cut off, empty.
pub(crate) fn reply(p: &Profile, result: Option<&Value>, out: &str, err: &str, wrote: Option<String>, whole: bool) -> Result<String> {
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
