//! Echo plugins: capabilities, described for the user (split out of `plugins.rs`, moved as it was).

use super::*;

// ── capabilities ──

/// What a plugin may ask of Echo. `ready` = this Echo implements it; a plugin using one that isn't
/// is listed but can't be installed ("needs a newer Echo"). The words are the permission sheet's —
/// written here, never taken from the plugin.
pub(super) struct Capability {
    id: &'static str,
    ready: bool,
    describe: fn(&Value) -> Result<String>,
}

pub(super) const CAPABILITIES: &[Capability] = &[
    Capability { id: "claude", ready: true, describe: describe_claude },
    Capability { id: "schedule", ready: true, describe: describe_schedule },
    Capability { id: "deliver", ready: true, describe: describe_deliver },
    Capability { id: "deliver_provider", ready: true, describe: describe_deliver_provider },
    Capability { id: "network", ready: true, describe: describe_network },
    Capability { id: "channel", ready: true, describe: describe_channel },
    Capability { id: "brain_source", ready: false, describe: describe_brain_source },
    Capability { id: "export", ready: false, describe: describe_export },
    Capability { id: "sessions", ready: true, describe: describe_sessions },
    Capability { id: "folders", ready: true, describe: describe_folders },
    Capability { id: "echo", ready: true, describe: describe_echo },
    Capability { id: "events", ready: true, describe: describe_events },
    Capability { id: "files", ready: true, describe: describe_files },
    Capability { id: "work", ready: true, describe: describe_work },
    Capability { id: "signin", ready: true, describe: describe_signin },
];

/// `uses.work: {}`: its items (a review asked of you, an issue assigned to you) beside Echo's tasks, never in a ledger.
pub(super) fn describe_work(v: &Value) -> Result<String> {
    if !v.is_object() {
        bail!("“work” is {{}}");
    }
    Ok("Adds its items to Today → My work and to threads' Work".into())
}

/// What happens in Echo that a plugin may hear (`uses.events.on`, `ctx.on`), with the sheet's words. Payloads carry
/// ids and what a plugin can act on — a call's folder name and title, a meeting's id and subject, a thread's id —
/// never the call's words (a plugin reads those through `echo.read`).
pub const EVENTS: &[(&str, &str)] = &[
    // The words say what text comes with it (a subject, a title): the user agrees to that too.
    ("meeting_soon", "a meeting is about to start (its subject)"),
    ("call_started", "a Teams call starts"),
    ("call_ended", "a Teams call ends"),
    ("recording_started", "a recording starts (the call's title)"),
    ("recording_stopped", "a recording stops"),
    ("transcribed", "a call's transcript is ready"),
    ("minutes_written", "a call's minutes are written (the call's title)"),
    ("thread_synced", "a thread's work is updated from its calls and mail"),
    ("sources_changed", "Echo's sources (calls, mail, channels) change"),
    ("threads_changed", "a thread is renamed, merged or deleted"),
];

pub(super) fn describe_events(v: &Value) -> Result<String> {
    let on = strings(v, "on")?;
    if on.is_empty() {
        bail!("“events” hears nothing ({})", EVENTS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "));
    }
    // As `echo`: one a newer Echo has is named on the sheet and never delivered here.
    let words: Vec<&str> = EVENTS.iter().filter(|(k, _)| on.iter().any(|o| o == k)).map(|(_, w)| *w).collect();
    let unknown: Vec<&str> = on.iter().map(String::as_str).filter(|o| !EVENTS.iter().any(|(k, _)| k == o)).collect();
    let mut lines = vec![];
    if !words.is_empty() {
        lines.push(format!("Knows when {}", join_words(&words)));
    }
    if !unknown.is_empty() {
        lines.push(format!("Asks for things this Echo doesn't have yet: {}", unknown.join(", ")));
    }
    Ok(lines.join(". "))
}

/// The events a plugin hears: those its `uses.events.on` names that Echo has; and, as before `events` existed,
/// `minutes_written` and `sources_changed` when its `echo.read` has sources (they say a source it may read is new) —
/// a plugin that reads only names or the dictionary doesn't hear them.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn heard_events(uses: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut out: Vec<String> = uses.get("events").map(|e| strings(e, "on").unwrap_or_default()).unwrap_or_default().into_iter().filter(|o| EVENTS.iter().any(|(k, _)| k == o)).collect();
    if uses.get("echo").is_some_and(|e| e["read"].as_array().is_some_and(|r| r.iter().any(|k| k == "source" || k == "sources"))) {
        for k in ["minutes_written", "sources_changed"] {
            if !out.iter().any(|o| o == k) {
                out.push(k.into());
            }
        }
    }
    out
}

pub(super) fn strings(v: &Value, key: &str) -> Result<Vec<String>> {
    match &v[key] {
        Value::Null => Ok(vec![]),
        Value::Array(a) => a.iter().map(|x| x.as_str().map(str::to_string).ok_or_else(|| anyhow!("“{key}” must list words"))).collect(),
        _ => bail!("“{key}” must be a list"),
    }
}

pub(super) fn only(what: &str, got: &[String], allowed: &[&str]) -> Result<()> {
    match got.iter().find(|g| !allowed.contains(&g.as_str())) {
        Some(g) => bail!("{what} “{g}” isn't one Echo knows ({})", allowed.join(", ")),
        None => Ok(()),
    }
}

pub(super) fn join_words(w: &[&str]) -> String {
    match w {
        [] => String::new(),
        [a] => a.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub(super) fn describe_claude(v: &Value) -> Result<String> {
    Ok(if v["skip"].as_bool() == Some(true) {
        "Runs Claude Code on your AI profiles, with the permissions you pick — or with every permission check skipped".into()
    } else {
        "Runs Claude Code on your AI profiles, with the permissions you pick".into()
    })
}

/// May a plugin with these `uses` start a Claude Code run with `tools` (`claudejob::Spec`)? Only with `claude` on its
/// install sheet; skipping every permission check only when the sheet said so too (`claude.skip`).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn may_run_claude(uses: &serde_json::Map<String, Value>, tools: &str) -> Result<()> {
    let Some(c) = uses.get("claude") else { bail!("it didn't ask to run Claude Code when it was installed") };
    if tools == "skip" && c["skip"].as_bool() != Some(true) {
        bail!("it didn't ask to run Claude Code with permission checks skipped when it was installed");
    }
    Ok(())
}

/// May a plugin with these `uses` send by `kind`? Only a way its install sheet named (`deliver.to`), or its own (the
/// kinds it provides, `deliver_provider.kinds`).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn may_send(uses: &serde_json::Map<String, Value>, kind: &str) -> Result<()> {
    let named = |cap: &str, key: &str| uses.get(cap).and_then(|v| v[key].as_array()).is_some_and(|a| a.iter().any(|k| k.as_str() == Some(kind)));
    if named("deliver", "to") || named("deliver_provider", "kinds") {
        Ok(())
    } else {
        bail!("it didn't ask to send by “{kind}” when it was installed")
    }
}

pub(super) fn describe_schedule(_: &Value) -> Result<String> {
    Ok("Works on its own on a schedule while Echo runs".into())
}

pub(super) fn describe_deliver(v: &Value) -> Result<String> {
    let to = strings(v, "to")?;
    if let Some(k) = to.iter().find(|k| !crate::deliver::valid_kind(k)) {
        bail!("a delivery “{k}” isn't a plain name");
    }
    if to.is_empty() {
        bail!("“deliver” names no way to send (teams, mail, or one a plugin adds)");
    }
    let words: Vec<String> = to
        .iter()
        .map(|t| match t.as_str() {
            "teams" => "Teams (your chat with yourself)".to_string(),
            "today" => "Echo's Today page".to_string(),
            "mail" => "mail (to you, or to the addresses you set on a routine)".to_string(),
            k => format!("{} (through the plugin that adds it)", title_case(k)),
        })
        .collect();
    Ok(format!("Sends messages: {}", join_words(&words.iter().map(String::as_str).collect::<Vec<_>>())))
}

pub(super) fn title_case(k: &str) -> String {
    let mut c = k.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

pub(super) fn describe_deliver_provider(v: &Value) -> Result<String> {
    let kinds = strings(v, "kinds")?;
    if kinds.is_empty() || kinds.iter().any(|k| !crate::deliver::valid_kind(k) || crate::deliver::BUILTIN.contains(&k.as_str())) {
        bail!("“deliver_provider” names its ways by plain new names");
    }
    Ok(format!("Adds a way Echo sends things to you: {}", join_words(&kinds.iter().map(|k| title_case(k)).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>())))
}

pub(super) fn describe_network(v: &Value) -> Result<String> {
    let hosts = strings(v, "hosts")?;
    let from = strings(v, "hosts_from")?;
    if (hosts.is_empty() && from.is_empty()) || hosts.iter().any(|h| h.is_empty() || !h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') || !h.contains('.')) {
        bail!("“network” names hosts (e.g. api.example.com) or settings that hold an address (hosts_from)");
    }
    let mut parts = vec![];
    if !hosts.is_empty() {
        parts.push(format!("{} over HTTPS", join_words(&hosts.iter().map(String::as_str).collect::<Vec<_>>())));
    }
    if !from.is_empty() {
        let quoted: Vec<String> = from.iter().map(|k| format!("“{k}”")).collect();
        let s = if from.len() == 1 { "setting" } else { "settings" };
        parts.push(format!("the addresses you list in its {s} {} (wildcards allowed; plain http only inside your own network)", join_words(&quoted.iter().map(String::as_str).collect::<Vec<_>>())));
    }
    Ok(format!("Talks to {}", parts.join(" and to ")))
}

pub(super) fn describe_channel(v: &Value) -> Result<String> {
    let p = v["plugin"].as_str().unwrap_or("");
    // Claude Code's own channel plugins only, from Anthropic's marketplace (an org allowlist would
    // refuse any other anyway).
    let Some(name) = p.strip_suffix("@claude-plugins-official").filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')) else {
        bail!("“channel” must name a channel plugin of claude-plugins-official, e.g. telegram@claude-plugins-official");
    };
    let shown: String = name.split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect::<Vec<_>>().join(" ");
    Ok(format!("Lets you reach your Claude · Echo folder session through {shown} while it is open in Echo"))
}

pub(super) fn describe_brain_source(v: &Value) -> Result<String> {
    match v["kind"].as_str() {
        Some("vault") => Ok("Adds a folder you pick (an Obsidian vault) to Recall: read and indexed on this computer".into()),
        _ => bail!("“brain_source” kind must be one Echo knows (vault)"),
    }
}

pub(super) fn describe_export(v: &Value) -> Result<String> {
    if v["kind"].as_str() != Some("vault") {
        bail!("“export” kind must be one Echo knows (vault)");
    }
    let what = strings(v, "what")?;
    only("An export of", &what, &["minutes", "people"])?;
    if what.is_empty() {
        bail!("“export” names nothing to write (minutes, people)");
    }
    Ok(format!("Writes copies of your {} into a folder you pick — only files it wrote, never yours", join_words(&what.iter().map(String::as_str).collect::<Vec<_>>())))
}

pub(super) fn describe_sessions(v: &Value) -> Result<String> {
    let mut parts = vec![];
    if v["skills"].as_bool() == Some(true) {
        parts.push("gives Claude in Echo skills of its own — text only, in Echo's sessions, not your Claude Code".to_string());
    }
    if v["read"].as_bool() == Some(true) {
        parts.push("lets Claude in Echo read the folders you give it (your Echo folder session may also change them, with your permissions)".to_string());
    }
    if v["hints"].as_bool() == Some(true) {
        parts.push("tells Claude what it knows about the client or thread open in the window".to_string());
    }
    let commands = strings(v, "commands")?;
    if commands.len() > 20 || commands.iter().any(|c| c.is_empty() || c.len() > 40 || c.starts_with('_') || !c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')) {
        bail!("“sessions” commands: up to 20 of its own command names (letters, digits, - and _)");
    }
    if !commands.is_empty() {
        let quoted: Vec<String> = commands.iter().map(|c| format!("“{c}”")).collect();
        parts.push(format!("lets Claude in Echo run its commands {} without asking", join_words(&quoted.iter().map(String::as_str).collect::<Vec<_>>())));
    }
    if parts.is_empty() {
        bail!("“sessions” adds nothing (skills, read, hints, commands)");
    }
    let mut text = parts.join("; ");
    text[..1].make_ascii_uppercase();
    Ok(text)
}

/// `uses.files.read`: other apps' own files the plugin reads, each named in full (`~/…`, `%APPDATA%/…`,
/// `%LOCALAPPDATA%/…`) — a list of apps' settings, never a folder, a pattern or a secret's place.
pub(super) fn describe_files(v: &Value) -> Result<String> {
    let files = strings(v, "read")?;
    if files.is_empty() || files.len() > 5 {
        bail!("“files” reads 1 to 5 files of other apps (e.g. ~/Library/Application Support/obsidian/obsidian.json)");
    }
    for f in &files {
        crate::plugins::app_file_rel(f)?;
    }
    let quoted: Vec<String> = files.iter().map(|f| format!("“{f}”")).collect();
    Ok(format!("Reads these files of other apps on this computer: {}", join_words(&quoted.iter().map(String::as_str).collect::<Vec<_>>())))
}

/// `uses.signin.hosts`: the hosts whose sign-in saved on this computer (the GitHub CLI's token, or the password or
/// token git's credential helper keeps — for a self-hosted git maybe the user's own password) the plugin's host half
/// may be handed — 1 to 4 plain host names, each once. It grants no network: the plugin names its API hosts in
/// `network.hosts` itself.
pub(super) fn describe_signin(v: &Value) -> Result<String> {
    let hosts = strings(v, "hosts")?;
    let once = hosts.iter().collect::<std::collections::BTreeSet<_>>().len() == hosts.len();
    if hosts.is_empty() || hosts.len() > 4 || !once || hosts.iter().any(|h| !plain_host(h)) {
        bail!("“signin” names 1 to 4 hosts, each once, by their plain names (e.g. github.com)");
    }
    Ok(format!(
        "Gets the sign-in saved on this computer for {} (GitHub CLI or git): it can act as you there and send it to the places it talks to",
        join_words(&hosts.iter().map(String::as_str).collect::<Vec<_>>())
    ))
}

/// A host's plain name: lowercase letters, digits, `-` and dots between labels, at least one dot (`github.com`), and
/// not an address (`10.0.0.1`: its last label all digits).
fn plain_host(h: &str) -> bool {
    h.len() <= 253
        && h.contains('.')
        && h.split('.').all(|l| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        && !h.rsplit('.').next().is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()))
}

/// May a plugin with these `uses` be handed the user's sign-in to `host`? Only one its install sheet named.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn may_use_signin(uses: &serde_json::Map<String, Value>, host: &str) -> Result<()> {
    if uses.get("signin").and_then(|v| v["hosts"].as_array()).is_some_and(|a| a.iter().any(|h| h.as_str() == Some(host))) {
        Ok(())
    } else {
        bail!("it didn't ask to use your sign-in to “{host}” when it was installed (uses.signin.hosts)")
    }
}

pub(super) fn describe_folders(v: &Value) -> Result<String> {
    let what = v["what"].as_str().unwrap_or("folders").trim();
    if what.is_empty() || what.chars().count() > 60 {
        bail!("“folders” what: a few words, e.g. “Obsidian vaults”");
    }
    Ok(if v["write"].as_bool() == Some(true) {
        format!("Reads and changes the {what} you pick, on this computer — only those: it may write, move and delete files in them")
    } else {
        format!("Reads the {what} you pick, on this computer — only those")
    })
}

/// What `uses.echo.read` may name, with how the sheet says it.
pub(super) const ECHO_READS: &[(&str, &str)] = &[
    ("clients", "the names of your clients"),
    ("threads", "the names of your threads"),
    ("people", "who the people in your threads are"),
    ("dictionary", "your dictionary"),
    ("sources", "your calls' minutes and transcripts and the facts of your threads' mail and channels"),
    ("source", "your calls' minutes and transcripts and the facts of your threads' mail and channels"),
    ("calls", "your calls' minutes and transcripts"),
];

pub(super) fn describe_echo(v: &Value) -> Result<String> {
    let read = strings(v, "read")?;
    let write = strings(v, "write")?;
    if read.is_empty() && write.is_empty() {
        bail!("“echo” reads nothing ({})", ECHO_READS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "));
    }
    // A kind a newer Echo has means nothing here: named on the sheet, never granted (an older Echo degrades, the
    // plugin still installs).
    let mut said: Vec<&str> = vec![];
    for k in &read {
        if let Some((_, words)) = ECHO_READS.iter().find(|(x, _)| x == k) {
            if !said.contains(words) {
                said.push(words);
            }
        }
    }
    let unknown: Vec<&str> = read.iter().chain(&write).map(String::as_str).filter(|k| !ECHO_READS.iter().any(|(x, _)| x == k) && *k != "dictionary").collect();
    let mut lines = vec![];
    if !said.is_empty() {
        lines.push(format!("Reads {} in Echo", join_words(&said)));
    }
    if write.iter().any(|w| w == "dictionary") {
        lines.push("Adds names to your Echo dictionary".to_string());
    }
    if !unknown.is_empty() {
        lines.push(format!("Asks for things this Echo doesn't have yet: {}", unknown.join(", ")));
    }
    Ok(lines.join(". "))
}

/// The permission sheet's lines for a manifest's `uses`, in order; an unknown capability or a bad
/// value is an error naming it.
pub fn describe(uses: &serde_json::Map<String, Value>) -> Result<Vec<String>> {
    uses.iter()
        .map(|(k, v)| {
            let c = CAPABILITIES.iter().find(|c| c.id == k).ok_or_else(|| anyhow!("it uses “{k}”, which this Echo doesn't know — it may need a newer Echo"))?;
            (c.describe)(v).with_context(|| format!("“{k}”"))
        })
        .collect()
}

/// The capabilities a manifest uses that this Echo doesn't implement yet.
pub(super) fn not_ready(uses: &serde_json::Map<String, Value>) -> Vec<String> {
    uses.keys().filter(|k| CAPABILITIES.iter().find(|c| c.id == k.as_str()).is_none_or(|c| !c.ready)).cloned().collect()
}
