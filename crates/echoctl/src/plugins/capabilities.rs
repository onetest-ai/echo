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
    Capability { id: "scripts", ready: true, describe: super::scripts::describe_scripts },
    Capability { id: "ai", ready: true, describe: describe_ai },
    Capability { id: "microphone", ready: true, describe: describe_microphone },
    Capability { id: "m365", ready: true, describe: describe_m365 },
    Capability { id: "me", ready: true, describe: describe_me },
    Capability { id: "skills", ready: true, describe: describe_skills },
];

/// The sheet's line for `uses.me` (`ctx.me`, `share::about_me`).
pub const ME_LINE: &str = "Reads your About me in Echo (Settings → About me): your name and role, the people and terms of your calls, how you like things written and how you work";

/// `uses.me: {}`: its host half reads About me, read-only.
pub(super) fn describe_me(v: &Value) -> Result<String> {
    if !v.as_object().is_some_and(|o| o.is_empty()) {
        bail!("“me” is {{}}");
    }
    Ok(ME_LINE.into())
}

/// The sheet's line for `uses.microphone` (`window::plugins` opens it only as it says).
pub const MICROPHONE_LINE: &str = "Hears your microphone in its own view: only after you click there, with Echo's recording sign on while it listens, and never while Echo records a call or a memo; it stops when you leave the view. What it hears can go to the addresses it may reach";

/// `uses.microphone: {}`: its window part hears the microphone, on the user's click in its view.
pub(super) fn describe_microphone(v: &Value) -> Result<String> {
    if !v.as_object().is_some_and(|o| o.is_empty()) {
        bail!("“microphone” is {{}}");
    }
    Ok(MICROPHONE_LINE.into())
}

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
    ("call_started", "a call starts (Teams, Google Meet)"),
    ("call_ended", "a call ends"),
    ("recording_started", "a recording starts (the call's title)"),
    ("recording_stopped", "a recording stops"),
    ("transcribed", "a call's transcript is ready"),
    ("minutes_written", "a call's minutes are written (the call's title)"),
    ("thread_synced", "a thread's work is updated from its calls and mail"),
    ("sources_changed", "Echo's sources (calls, mail, channels) change"),
    ("threads_changed", "a thread is renamed, merged or deleted"),
    ("shared_changed", "a value its marketplace's plugins share changes"),
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
    if reads_sources(uses) {
        for k in ["minutes_written", "sources_changed"] {
            if !out.iter().any(|o| o == k) {
                out.push(k.into());
            }
        }
    }
    out
}

/// The plugin may read Echo's sources (`uses.echo.read` names `source` or `sources`: the same data).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn reads_sources(uses: &serde_json::Map<String, Value>) -> bool {
    uses.get("echo").is_some_and(|e| e["read"].as_array().is_some_and(|r| r.iter().any(|k| k == "source" || k == "sources")))
}

/// What of an event only a plugin that reads Echo's sources hears (`reads_sources`): a call's routing, people and
/// minutes; a mail's or post's subject, senders and routing. Another plugin hearing the event gets it without them.
const FOR_READERS: &[(&str, &[&str])] = &[("minutes_written", &["threads", "client", "profile", "participants", "minutes"]), ("sources_changed", &["items"])];

/// An event's payload as this plugin hears it: without what only readers of sources hear (`FOR_READERS`) unless `reads`.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn as_heard(event: &str, payload: &mut Value, reads: bool) {
    if reads {
        return;
    }
    if let (Some((_, keys)), Some(o)) = (FOR_READERS.iter().find(|(e, _)| *e == event), payload.as_object_mut()) {
        for k in *keys {
            o.remove(*k);
        }
    }
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

/// Its runs are routines: they run as your Claude · Echo folder session, by your one choice — `claude.skip` (from
/// before that choice) is still taken and changes nothing. In a folder you gave it, a run works there on that folder's
/// own setup, its permissions as you set routines too; a session the user opens there from its screens, as they set
/// Claude in the window.
/// The sheet's line for `uses.skills: true` (`claude_skills`: each skill only on the user's click).
pub const SKILLS_LINE: &str = "Adds instructions for Claude Code (skills) to your own Claude Code (~/.claude/skills), each only when you click to add it: skills that run commands are refused, never over a skill that is there already, changing or removing only the skills it added — they go when you remove the plugin";

/// `uses.skills: true`: its window half adds skills to the user's own Claude Code, each on a click.
pub(super) fn describe_skills(v: &Value) -> Result<String> {
    if *v != Value::Bool(true) {
        bail!("“skills” is true");
    }
    Ok(SKILLS_LINE.into())
}

pub(super) fn describe_claude(v: &Value) -> Result<String> {
    let mut line = "Runs Claude Code on your AI profiles, acting on its own as you set routines in Settings (Auto, or skipping every permission check)".to_string();
    if v["skip"] == true {
        line.push_str("; a run you mark so may skip every permission check");
    }
    line.push_str(
        ". In a folder you gave it, Claude works with that folder's own Claude Code setup: it reads the folder and, if the plugin may change it, \
         edits files there or in a copy for you to review — as you set routines; on your click in its screens it opens Claude Code there in Echo's Claude panel for you to ask, as you set Claude in the window",
    );
    Ok(line)
}

/// May a plugin with these `uses` start a Claude Code run (`claudejob::Spec`)? Only with `claude` on its install sheet.
/// The `tools` it may ask (`claudejob::Tools`): `skip` only when its sheet said so (`claude.skip`), else the user's
/// setting for routines (`""`); anything else as it asked.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn may_run_claude<'a>(uses: &serde_json::Map<String, Value>, tools: &'a str) -> Result<&'a str> {
    let Some(claude) = uses.get("claude") else { bail!("it didn't ask to run Claude Code when it was installed") };
    Ok(if tools.trim() == "skip" && claude["skip"] != true { "" } else { tools })
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
    // `ask`: it may ask the user, in Echo's own box, for an address to fetch once (`ctx.http.askAddress`).
    let ask = match v.get("ask") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => bail!("“network.ask” is true or false"),
    };
    if (hosts.is_empty() && from.is_empty() && !ask) || hosts.iter().any(|h| h.is_empty() || !h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') || !h.contains('.')) {
        bail!("“network” names hosts (e.g. api.example.com), settings that hold an address (hosts_from) or asks you for one (ask)");
    }
    let asks = "Fetches a web page from an address you type in Echo's own box when it asks — that page only, once each time";
    if hosts.is_empty() && from.is_empty() {
        return Ok(asks.into());
    }
    let mut parts = vec![];
    if !hosts.is_empty() {
        parts.push(format!("{} over HTTPS", join_words(&hosts.iter().map(String::as_str).collect::<Vec<_>>())));
    }
    if !from.is_empty() {
        let quoted: Vec<String> = from.iter().map(|k| format!("“{k}”")).collect();
        let s = if from.len() == 1 { "setting" } else { "settings" };
        parts.push(format!("the addresses you list in its {s} {} (wildcards allowed, plain http too; its window part also over WebSocket)", join_words(&quoted.iter().map(String::as_str).collect::<Vec<_>>())));
    }
    let talks = format!("Talks to {}", parts.join(" and to "));
    Ok(if ask { format!("{talks}. {asks}") } else { talks })
}

/// What `uses.m365.read` may name, with the sheet's words for each: what it is, and what of it is read.
const M365_READS: &[(&str, &str, &str)] = &[("mail", "mail", "a mail's thread with its attachments' names"), ("chats", "Teams chats", "a chat's messages over some days")];

/// `uses.m365.read`: the user's Microsoft 365, read-only through Echo's own sign-in (`ctx.m365`); nothing sent.
pub(super) fn describe_m365(v: &Value) -> Result<String> {
    let read = strings(v, "read")?;
    let asked: Vec<&(&str, &str, &str)> = M365_READS.iter().filter(|(k, _, _)| read.iter().any(|r| r == k)).collect();
    let unknown: Vec<&str> = read.iter().map(String::as_str).filter(|r| !M365_READS.iter().any(|(k, _, _)| k == r)).collect();
    if asked.is_empty() && unknown.is_empty() {
        bail!("“m365” reads nothing (read: {})", M365_READS.iter().map(|(k, _, _)| *k).collect::<Vec<_>>().join(", "));
    }
    let mut lines = vec![];
    if !asked.is_empty() {
        let what = join_words(&asked.iter().map(|(_, w, _)| *w).collect::<Vec<_>>());
        let reads = asked.iter().map(|(_, _, r)| *r).collect::<Vec<_>>().join(" or ");
        lines.push(format!("Reads your Microsoft 365 {what} through Echo's sign-in when it asks: it searches them and reads {reads} — it never sends"));
    }
    if !unknown.is_empty() {
        lines.push(format!("Asks for things this Echo doesn't have yet: {}", unknown.join(", ")));
    }
    Ok(lines.join(". "))
}

pub(super) fn describe_channel(v: &Value) -> Result<String> {
    let p = v["plugin"].as_str().unwrap_or("");
    // Claude Code's own channel plugins only, from Anthropic's marketplace (an org allowlist would
    // refuse any other anyway).
    let Some(name) = p.strip_suffix("@claude-plugins-official").filter(|n| n.split('-').all(|w| !w.is_empty()) && n.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')) else {
        bail!("“channel” must name a channel plugin of claude-plugins-official, e.g. telegram@claude-plugins-official");
    };
    let shown: String = name.split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect::<Vec<_>>().join(" ");
    Ok(format!("Lets you reach your Claude · Echo folder session through {shown} while it is open in Echo, and installs Claude Code's {p} plugin for it"))
}

/// `uses.ai.wrapper` `{name, command}`: a wrapper profile it offers (echo-plugins R24) — one program name, no
/// arguments, found on the user's PATH when they add it (never a file of the plugin: `ai::wrapper::admit`).
pub(super) fn describe_ai(v: &Value) -> Result<String> {
    let w = &v["wrapper"];
    if !w.is_object() || v.as_object().is_some_and(|o| o.keys().any(|k| k != "wrapper")) {
        bail!("“ai” offers only a wrapper: {{\"wrapper\": {{\"name\": …, \"command\": …}}}}");
    }
    if w.as_object().is_some_and(|o| o.keys().any(|k| k != "name" && k != "command")) {
        bail!("“ai.wrapper” holds only its name and command");
    }
    let command = w["command"].as_str().filter(|c| crate::ai::wrapper::program_name(c)).ok_or_else(|| anyhow!("“ai.wrapper.command” is one program name (letters, digits, “.”, “_”, “-”) with no arguments, never a path"))?;
    wrapper_name(w).ok_or_else(|| anyhow!("“ai.wrapper.name” is a name of up to 40 characters"))?;
    Ok(format!("Offers an AI profile that starts {command} — a program you install yourself — in place of Claude Code; it sees and can change everything Claude in Echo is sent, call transcripts included. You add it in Settings → AI & minutes, if you want it"))
}

/// A wrapper offer's name: up to 40 characters, one line.
pub(crate) fn wrapper_name(w: &Value) -> Option<String> {
    w["name"].as_str().map(str::trim).filter(|n| !n.is_empty() && n.chars().count() <= 40 && !n.chars().any(char::is_control)).map(str::to_owned)
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
    let named = join_words(&hosts.iter().map(String::as_str).collect::<Vec<_>>());
    let mut line = format!("Gets the sign-in saved on this computer for {named} (GitHub CLI or git): it can act as you there and send it to the places it talks to");
    match v.get("keep") {
        None | Some(Value::Bool(false)) => {}
        Some(Value::Bool(true)) => line.push_str(&format!("; it may sign you in to {named} itself and have Echo keep that sign-in for it")),
        Some(_) => bail!("“signin” keep is true or false"),
    }
    Ok(line)
}

/// A host's plain name: lowercase letters, digits, `-` and dots between labels, at least one dot (`github.com`), and
/// not an address (`10.0.0.1`: its last label all digits).
fn plain_host(h: &str) -> bool {
    h.len() <= 253
        && h.contains('.')
        && h.split('.').all(|l| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        && !h.rsplit('.').next().is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()))
}

/// Did its sheet say Echo keeps a sign-in it makes (`uses.signin.keep`)?
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn may_keep_signin(uses: &serde_json::Map<String, Value>) -> bool {
    uses.get("signin").is_some_and(|v| v["keep"] == Value::Bool(true))
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
    ("threads", "your threads' names and stages"),
    ("people", "who the people in your threads are (their names, addresses and organisations)"),
    ("dictionary", "your dictionary"),
    ("sources", "your calls' minutes, transcripts, meeting chat and who was invited, and the facts of your threads' mail and channels"),
    ("source", "your calls' minutes, transcripts, meeting chat and who was invited, and the facts of your threads' mail and channels"),
    ("calls", "the list of your calls (titles, times, who was invited with their addresses, their threads)"),
    ("calendar", "the meetings Echo has read from your calendar"),
    ("me", "your name and email"),
];

/// `uses.echo.jobs` on the sheet (echo-plugins R25).
const JOBS_LINE: &str = "Shows Echo's background jobs and changes how often each runs, when it pauses at night and which model it uses — only when you change them in its screens";

/// What `uses.echo.write` may name, with the sheet's line for each.
pub(super) const ECHO_WRITES: &[(&str, &str)] = &[
    ("dictionary", "Adds names to your Echo dictionary"),
    ("thread_stage", "Changes your threads' stage"),
    ("tasks", "Adds and updates tasks in your threads"),
];

pub(super) fn describe_echo(v: &Value) -> Result<String> {
    let read = strings(v, "read")?;
    let write = strings(v, "write")?;
    // `jobs`: it shows Echo's background jobs and tunes them on the user's own click or key (echo-plugins R25).
    let jobs = match v.get("jobs") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => bail!("“echo.jobs” is true or false"),
    };
    if read.is_empty() && write.is_empty() && !jobs {
        bail!("“echo” asks for nothing: read ({}), write ({}) or jobs", ECHO_READS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "), ECHO_WRITES.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "));
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
    let unknown: Vec<&str> = read.iter().filter(|k| !ECHO_READS.iter().any(|(x, _)| *x == k.as_str())).chain(write.iter().filter(|k| !ECHO_WRITES.iter().any(|(x, _)| *x == k.as_str()))).map(String::as_str).collect();
    let mut lines = vec![];
    if !said.is_empty() {
        lines.push(format!("Reads {} in Echo", join_words(&said)));
    }
    for (k, line) in ECHO_WRITES {
        if write.iter().any(|w| w == k) {
            lines.push(line.to_string());
        }
    }
    if jobs {
        lines.push(JOBS_LINE.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_asks_for_about_me_as_an_empty_object_and_the_sheet_says_all_it_may_read() {
        let uses = serde_json::json!({"me": {}});
        assert_eq!(describe(uses.as_object().unwrap()).unwrap(), vec![ME_LINE.to_string()]);
        assert!(ME_LINE.contains("About me") && ME_LINE.contains("how you like things written") && ME_LINE.contains("how you work"));
        assert!(not_ready(uses.as_object().unwrap()).is_empty(), "this Echo has it");
        for bad in [serde_json::json!(true), serde_json::json!({"kinds": ["style"]}), serde_json::json!("yes")] {
            assert!(describe_me(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_plugin_asks_for_the_microphone_as_an_empty_object_and_the_sheet_says_when_it_opens() {
        let uses = serde_json::json!({"microphone": {}});
        assert_eq!(describe(uses.as_object().unwrap()).unwrap(), vec![MICROPHONE_LINE.to_string()]);
        assert!(MICROPHONE_LINE.contains("only after you click there") && MICROPHONE_LINE.contains("never while Echo records"));
        assert!(not_ready(uses.as_object().unwrap()).is_empty(), "this Echo has it");
        for bad in [serde_json::json!(true), serde_json::json!({"always": true}), serde_json::json!("yes")] {
            assert!(describe_microphone(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_sheet_says_each_write_to_echos_data_and_names_one_it_doesnt_know() {
        let line = describe_echo(&serde_json::json!({"read": ["threads"], "write": ["thread_stage", "tasks"]})).unwrap();
        assert_eq!(line, "Reads your threads' names and stages in Echo. Changes your threads' stage. Adds and updates tasks in your threads");
        let only = describe_echo(&serde_json::json!({"write": ["tasks"]})).unwrap();
        assert_eq!(only, "Adds and updates tasks in your threads", "a sheet that adds one asks again: its line differs");
        assert!(describe_echo(&serde_json::json!({"write": ["calendar"]})).unwrap().contains("Asks for things this Echo doesn't have yet: calendar"));
    }

    #[test]
    fn the_sheet_names_each_of_echos_calls_calendar_and_you_that_a_plugin_reads() {
        let line = describe_echo(&serde_json::json!({"read": ["calls", "source", "calendar", "me"]})).unwrap();
        assert_eq!(line, "Reads the list of your calls (titles, times, who was invited with their addresses, their threads), your calls' minutes, transcripts, meeting chat and who was invited, and the facts of your threads' mail and channels, the meetings Echo has read from your calendar and your name and email in Echo");
    }

    #[test]
    fn a_plugin_reads_microsoft_365_only_as_its_sheet_says_and_never_sends() {
        assert_eq!(describe_m365(&serde_json::json!({"read": ["mail"]})).unwrap(), "Reads your Microsoft 365 mail through Echo's sign-in when it asks: it searches them and reads a mail's thread with its attachments' names — it never sends");
        assert_eq!(
            describe_m365(&serde_json::json!({"read": ["mail", "chats"]})).unwrap(),
            "Reads your Microsoft 365 mail and Teams chats through Echo's sign-in when it asks: it searches them and reads a mail's thread with its attachments' names or a chat's messages over some days — it never sends"
        );
        assert!(describe_m365(&serde_json::json!({"read": ["files"]})).unwrap().contains("Asks for things this Echo doesn't have yet: files"), "named, never granted");
        assert!(describe_m365(&serde_json::json!({"read": []})).is_err());
        assert!(describe_m365(&serde_json::json!({"send": ["mail"]})).is_err(), "no sends");
    }

    #[test]
    fn a_plugin_that_asks_for_a_typed_address_says_so_on_its_sheet() {
        let ask = "Fetches a web page from an address you type in Echo's own box when it asks — that page only, once each time";
        assert_eq!(describe_network(&serde_json::json!({"ask": true})).unwrap(), ask);
        assert_eq!(describe_network(&serde_json::json!({"hosts": ["api.acme.example"], "ask": true})).unwrap(), format!("Talks to api.acme.example over HTTPS. {ask}"));
        assert!(!describe_network(&serde_json::json!({"hosts": ["api.acme.example"]})).unwrap().contains("type"));
        assert!(describe_network(&serde_json::json!({"ask": false})).is_err(), "asks for nothing");
        assert!(describe_network(&serde_json::json!({"ask": "yes"})).is_err());
    }

    #[test]
    fn a_channel_names_one_of_anthropics_plugins_and_a_bad_name_is_refused_not_a_panic() {
        assert!(describe_channel(&serde_json::json!({"plugin": "telegram@claude-plugins-official"})).unwrap().contains("Telegram"));
        for bad in ["telegram-@claude-plugins-official", "-x@claude-plugins-official", "a--b@claude-plugins-official", "@claude-plugins-official", "x@other"] {
            assert!(describe_channel(&serde_json::json!({"plugin": bad})).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_plugin_asks_for_echos_jobs_as_a_flag_and_the_sheet_says_what_it_can_change() {
        assert_eq!(describe_echo(&serde_json::json!({"jobs": true})).unwrap(), JOBS_LINE);
        assert!(describe_echo(&serde_json::json!({"read": ["threads"], "jobs": true})).unwrap().ends_with(JOBS_LINE));
        assert!(describe_echo(&serde_json::json!({"jobs": false})).is_err(), "nothing asked for");
        assert!(describe_echo(&serde_json::json!({"jobs": "yes"})).is_err());
        assert!(!describe_echo(&serde_json::json!({"read": ["threads"]})).unwrap().contains("background jobs"));
    }

    #[test]
    fn a_plugin_offers_a_wrapper_as_one_program_name_and_the_sheet_says_what_it_sees() {
        let line = describe_ai(&serde_json::json!({"wrapper": {"name": "Acme Claude", "command": "acme-claude"}})).unwrap();
        assert_eq!(line, "Offers an AI profile that starts acme-claude — a program you install yourself — in place of Claude Code; it sees and can change everything Claude in Echo is sent, call transcripts included. You add it in Settings → AI & minutes, if you want it");
        let other = describe_ai(&serde_json::json!({"wrapper": {"name": "Acme Claude", "command": "acme-claude2"}})).unwrap();
        assert_ne!(line, other, "an update that changes the program asks again: its line differs");
        for bad in [
            serde_json::json!({"wrapper": {"name": "A", "command": "acme-claude --x"}}),
            serde_json::json!({"wrapper": {"name": "A", "command": "./bin/acme-claude"}}),
            serde_json::json!({"wrapper": {"name": "A", "command": "/usr/local/bin/acme-claude"}}),
            serde_json::json!({"wrapper": {"name": "A", "command": "node"}, "key": "x"}),
            serde_json::json!({"wrapper": {"name": "A", "command": "acme", "args": ["-x"]}}),
            serde_json::json!({"wrapper": {"name": "", "command": "acme"}}),
            serde_json::json!({"wrapper": {"name": "x".repeat(41), "command": "acme"}}),
            serde_json::json!({"wrapper": {"name": "A\nB", "command": "acme"}}),
            serde_json::json!({"gateway": "https://x"}),
        ] {
            assert!(describe_ai(&bad).is_err(), "{bad}");
        }
    }
}
