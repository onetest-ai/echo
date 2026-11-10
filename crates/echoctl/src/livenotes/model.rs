//! Live notes: talking to the model (split out of `livenotes.rs`, moved as it was).

use super::*;

// ── talking to the model ────────────────────────────────────────────────────────────────────

/// One completion: `system` + `user` → the model's text. `cancelled()` is polled while it runs (a
/// stopped job is a stopped child process). `whole`: a reply cut off at the token limit is an
/// error (a summary's JSON would be broken).
pub(super) fn complete(p: &Profile, system: &str, user: &str, max_tokens: u32, whole: bool, cancelled: &dyn Fn() -> bool) -> Result<String> {
    match p.kind {
        Kind::Local => complete_local(p, system, user, max_tokens, whole, cancelled),
        Kind::System | Kind::Claude => complete_claude(p, &p.live_model(), system, user, whole, CLAUDE_DEADLINE, cancelled),
    }
}

/// A model's JSON answer: the whole text, or what lies between its first `{` and last `}` (a code fence or words
/// around it allowed). None when there is none — never a panic, whatever order the braces come in.
pub(crate) fn json_answer(text: &str) -> Option<serde_json::Value> {
    let t = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    serde_json::from_str(t).ok().or_else(|| {
        let (a, b) = (t.find('{')?, t.rfind('}')?);
        (a < b).then(|| serde_json::from_str(&t[a..=b]).ok()).flatten()
    })
}

/// One prompt, one answer, no tools (a prep, the Brain's jobs, a diagram's repair): on `text_model(p)`.
pub(crate) fn complete_text(p: &Profile, system: &str, user: &str, max_tokens: u32) -> Result<String> {
    let model = text_model(p);
    match p.kind {
        Kind::Local => complete_local_body(p, &model, system, json!(user), max_tokens, true, &|| crate::notes::quitting()),
        Kind::System | Kind::Claude => complete_claude(p, &model, system, user, true, Duration::from_secs(300), &|| crate::notes::quitting()),
    }
}

/// The model a one-shot job runs on — what it records (#294). A local profile: its live model, the one already loaded
/// for live notes (these jobs run every few minutes, during calls too: a second, larger model would crowd it out);
/// else its minutes model.
pub(crate) fn text_model(p: &Profile) -> String {
    match p.kind {
        Kind::Local => p.live_model(),
        Kind::System | Kind::Claude => p.notes_model().unwrap_or_else(|| "sonnet".into()),
    }
}

/// One request with images (PNG bytes) and a text, on `text_model(p)`'s model: the Brain's page
/// reader. Claude Code gets the images as content blocks (no tools); a `local` profile its Messages API.
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
pub(crate) fn complete_vision(p: &Profile, system: &str, images: &[Vec<u8>], text: &str, max_tokens: u32) -> Result<String> {
    use base64::Engine as _;
    let mut blocks: Vec<Value> = images
        .iter()
        .map(|png| json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": base64::engine::general_purpose::STANDARD.encode(png)}}))
        .collect();
    blocks.push(json!({"type": "text", "text": text}));
    let blocks = Value::Array(blocks);
    match p.kind {
        Kind::Local => complete_local_body(p, &p.live_model(), system, blocks, max_tokens, true, &|| crate::notes::quitting()),
        Kind::System | Kind::Claude => {
            let model = p.notes_model().unwrap_or_else(|| "sonnet".into());
            crate::claude_session::complete_claude_with(p, &model, system, text, Some(&blocks), true, Duration::from_secs(300), &|| crate::notes::quitting())
        }
    }
}

pub(super) fn curl_quote(s: &str) -> String {
    // A newline would end the option and start another (curl reads its config line by line).
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

/// The curl config for one request (read from stdin: the token never on a command line). The HTTP
/// status comes last, on a line of its own.
pub(super) fn curl_config(p: &Profile, body: &Value) -> String {
    let base = p.base_url.as_deref().unwrap_or("").trim_end_matches('/');
    let token = p.token.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "ollama".into());
    let mut c = format!(
        "url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\nheader = \"anthropic-version: 2023-06-01\"\nheader = {}\ndata-binary = {}\nmax-time = {}\nconnect-timeout = 5\nwrite-out = \"\\n%{{http_code}}\"\n",
        curl_quote(&format!("{base}/v1/messages")),
        curl_quote(&format!("x-api-key: {token}")),
        curl_quote(&body.to_string()),
        LOCAL_DEADLINE.as_secs(),
    );
    // "This Mac" stays on this Mac, whatever proxy the environment names.
    if ai::loopback(base) {
        c.push_str("noproxy = \"*\"\n");
    }
    c
}

/// curl's arguments: `-q` must come first (it stops curl reading `~/.curlrc`); no `-L`.
pub(super) const CURL_ARGS: [&str; 4] = ["-q", "-sS", "-K", "-"];

/// The last non-empty line of a child's stderr (a login shell's start-up noise comes first).
pub(crate) fn last_line(s: &str) -> String {
    s.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("").to_string()
}

/// `POST <base_url>/v1/messages` with thinking off, through curl. `-q` first: the user's
/// `.curlrc` isn't read (it could turn on following redirects); no `-L`: redirects aren't followed.
/// Live notes on a local profile: its live model.
pub(super) fn complete_local(p: &Profile, system: &str, user: &str, max_tokens: u32, whole: bool, cancelled: &dyn Fn() -> bool) -> Result<String> {
    complete_local_body(p, &p.live_model(), system, json!(user), max_tokens, whole, cancelled)
}

pub(super) fn complete_local_body(p: &Profile, model: &str, system: &str, content: Value, max_tokens: u32, whole: bool, cancelled: &dyn Fn() -> bool) -> Result<String> {
    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "thinking": {"type": "disabled"},
        "system": system,
        "messages": [{"role": "user", "content": content}],
    });
    let mut c = Command::new("curl");
    #[cfg(windows)]
    {
        use crate::app::NoWindow;
        c = c.no_window();
    }
    c.args(CURL_ARGS).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    ai::detach(&mut c);
    let mut child = c.spawn().context("running curl")?;
    let (out, err, ok, wrote) = ai::wait_output_with(&mut child, Some(curl_config(p, &body).into_bytes()), LOCAL_DEADLINE + Duration::from_secs(5), cancelled)
        .map_err(|e| anyhow!("{}: {e:#}", p.name))?;
    if let Err(e) = wrote {
        bail!("{} didn't get the request: {e} {}", p.name, last_line(&err));
    }
    if !ok {
        let why = last_line(&err);
        bail!("{} didn't answer{}", p.name, if why.is_empty() { String::new() } else { format!(": {why}") });
    }
    let (body, status) = out.rsplit_once('\n').unwrap_or((out.as_str(), ""));
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => bail!("{} answered HTTP {}: {}", p.name, status.trim(), clip(body, 200)),
    };
    if let Some(e) = v.get("error") {
        bail!("{}: {}", p.name, e["message"].as_str().or(e.as_str()).unwrap_or("error"));
    }
    if status.trim().parse::<u16>().is_ok_and(|s| s >= 400) {
        bail!("{} answered HTTP {}: {}", p.name, status.trim(), clip(body, 200));
    }
    if whole && v["stop_reason"] == "max_tokens" {
        bail!("{}'s reply was cut off at the token limit", p.name);
    }
    let text: String = v["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "text").filter_map(|c| c["text"].as_str()).collect();
    if text.trim().is_empty() {
        bail!("{} returned no text", p.name);
    }
    Ok(text)
}

/// A lean Claude Code run: no tools, our system prompt, the text on stdin; the result envelope
/// is checked before the model's own text is used. `system` runs where the minutes run (the
/// data folder or `notes_in`: its `.claude/settings.json` is part of "as configured"); the others
/// in Echo's state folder (they load no project settings anyway).
pub(super) fn complete_claude(p: &Profile, model: &str, system: &str, user: &str, whole: bool, deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<String> {
    crate::claude_session::complete_claude_with(p, model, system, user, None, whole, deadline, cancelled)
}
