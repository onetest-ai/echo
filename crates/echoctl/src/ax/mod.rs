//! Reading the call app's window through the platform's accessibility API (macOS AX, Windows
//! UI Automation): `MuteWatch` follows the microphone button, `UiRecorder` logs the window's
//! text during a recording (raw material for naming remote speakers; text only, no
//! screenshots), and the dev tools `echoctl ax-dump` / `ax-mic`. Same API on both platforms.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

/// `ui.jsonl` stops growing past this.
const UI_LOG_MAX_BYTES: u64 = 40_000_000;
/// Full window trees saved at these offsets into the call (structure for building the reader).
const UI_TREES_AT: &[u64] = &[10, 120, 600];

/// Teams' microphone button (an HTML id: AutomationId on Windows, AXDOMIdentifier on macOS —
/// Teams is the same web app in WebView2 on both). The window holding it is the call window.
const MIC_BUTTON_ID: &str = "microphone-button";
/// On the stage while someone else shares (whatever Teams' language; absent while you present).
const POPOUT_ID: &str = "popout-content-button";
/// English only, and the only source of the presenter's name.
const SHARED_BY: &str = "Content shared by ";
const SHARE_BUTTON_ID: &str = "share-button";
/// The share button's label while you present (English only).
const STOP_SHARING: &str = "Stop sharing";

/// Screen sharing in the call.
#[derive(Debug, Clone, PartialEq)]
pub enum Sharing {
    Nobody,
    /// Someone else presents (their name when Teams is in English).
    Them(Option<String>),
    /// You present (detected in English Teams only).
    Me,
}

/// Who is presenting, folded from the call window's nodes (id + each of its labels).
#[derive(Default)]
struct ShareSeen {
    popout: bool,
    by: Option<String>,
    mine: bool,
}

impl ShareSeen {
    fn see(&mut self, id: &str, label: &str) {
        self.popout |= id == POPOUT_ID;
        self.mine |= id == SHARE_BUTTON_ID && label.starts_with(STOP_SHARING);
        if let Some(who) = label.strip_prefix(SHARED_BY) {
            self.by = Some(who.trim().to_string());
        }
    }

    fn sharing(self) -> Sharing {
        match self {
            ShareSeen { mine: true, .. } => Sharing::Me,
            ShareSeen { popout, by, .. } if popout || by.is_some() => Sharing::Them(by),
            _ => Sharing::Nobody,
        }
    }
}

/// Who the call is with, from the call window's title: Teams titles it after the chat it was
/// started from (a 1:1 chat: the other person; a group chat: its name) or the meeting, with
/// " | "-separated suffixes (tenant, account, "Microsoft Teams"). Returns the first part.
pub fn call_name(title: &str) -> Option<String> {
    let first = title.split(" | ").next()?.trim();
    let name = ["Call with ", "Meeting with ", "Meeting in "]
        .iter()
        .find_map(|p| first.strip_prefix(p))
        .unwrap_or(first)
        .trim_matches(|c: char| c == '"' || c == '“' || c == '”' || c.is_whitespace());
    let generic = ["", "Microsoft Teams", "Meeting", "Call", "Calls", "Chat", "Teams"];
    (!generic.iter().any(|g| name.eq_ignore_ascii_case(g)) && !name.contains('@')).then(|| name.to_string())
}

/// Looks like one person's name ("Jane Doe", "Mary-Ann O'Neil"): 2–4 capitalised words of
/// letters. Group chat names and meeting subjects usually aren't.
pub fn is_person_name(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    (2..=4).contains(&words.len())
        && words.iter().all(|w| {
            w.chars().next().is_some_and(char::is_uppercase) && w.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'' || c == '.')
        })
}

// ── who is lit: participant state for naming remote speakers ───────────────────────────────

/// A light copy of one node of the call window (both platforms): what the participant reader
/// needs. `id`: AXDOMIdentifier / AutomationId (the HTML id); `classes`: AXDOMClassList /
/// UIA ClassName (the HTML class attribute); `label`: the first of title / description / value
/// (macOS) or the name (Windows).
#[derive(Debug, Default, Clone)]
pub struct UiNode {
    pub role: String,
    pub id: String,
    pub classes: Vec<String>,
    pub label: String,
    pub kids: Vec<UiNode>,
}

/// One participant as the call window shows them right now: a video tile or a row of the
/// participants list. `tokens` = every class on the element, its subtree and (tiles) a few
/// ancestors, plus its label's state words ("t:muted") — Teams marks the speaker by toggling
/// some of these; which one is learned per call (`speaking.rs`), since Teams' class names are
/// generated and change between versions.
#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    pub key: String,
    pub name: String,
    /// The local user's own tile ("Myself video, <name>, …").
    pub me: bool,
    pub tokens: std::collections::BTreeSet<String>,
}

/// A tile's label → (name, is the user's own tile, the remaining parts). "Daniel Osei, video is
/// on, muted, …", "Myself video, Jane Doe, …", "Osei, Daniel (Contoso), …".
fn tile_name(label: &str) -> Option<(String, bool, Vec<String>)> {
    let parts: Vec<&str> = label.split(", ").map(str::trim).collect();
    if parts.len() == 1 {
        // The compact call view labels the other person's tile with just their name.
        return is_person_name(parts[0]).then(|| (parts[0].to_string(), false, vec![]));
    }
    let rest = |n: usize| parts[n..].iter().map(|p| p.to_lowercase()).collect();
    let capital = |p: &str| p.chars().next().is_some_and(char::is_uppercase);
    if is_person_name(parts[0]) {
        return Some((parts[0].to_string(), false, rest(1)));
    }
    if !parts[0].contains(' ') && capital(parts[0]) && capital(parts[1]) && (parts[1].contains('(') || !parts[1].contains(' ')) {
        return Some((format!("{}, {}", parts[0], parts[1]), false, rest(2)));
    }
    if parts.len() >= 3 && is_person_name(parts[1]) {
        return Some((parts[1].to_string(), true, rest(2)));
    }
    None
}

/// Classes of a subtree, and its element ids as `id:<id>` (a raised hand shows as one).
fn subtree_classes(n: &UiNode, depth: usize, out: &mut std::collections::BTreeSet<String>) {
    out.extend(n.classes.iter().cloned());
    if !n.id.is_empty() {
        out.insert(format!("id:{}", n.id));
    }
    if depth < 8 {
        for k in &n.kids {
            subtree_classes(k, depth + 1, out);
        }
    }
}

/// Teams' participants list item id (Teams' HTML; any language).
const ROSTER_ITEM: &str = "calling-roster-item-";

/// The participants the call window shows: its video tiles (menu items labelled with a name)
/// and, when the participants pane is open, its rows.
pub fn participants(root: &UiNode) -> Vec<Participant> {
    fn walk<'a>(n: &'a UiNode, chain: &mut Vec<&'a UiNode>, out: &mut Vec<Participant>) {
        let role = n.role.to_lowercase();
        if n.id.starts_with(ROSTER_ITEM) {
            let label = if n.label.is_empty() { chain.last().map(|p| p.label.as_str()).unwrap_or("") } else { &n.label };
            let name = label.split(", ").next().unwrap_or("").trim().to_string();
            if !name.is_empty() {
                let mut tokens = Default::default();
                subtree_classes(n, 0, &mut tokens);
                tokens.extend(label.split(", ").skip(1).map(|p| format!("t:{}", p.trim().to_lowercase())));
                out.push(Participant { key: format!("roster:{}", &n.id[ROSTER_ITEM.len()..]), name, me: false, tokens });
            }
        } else if role.contains("menuitem") || role.contains("image") {
            // Tiles are menu items; the user's own preview can be an image ("Myself video, …").
            let tile = tile_name(&n.label).filter(|t| role.contains("menuitem") || t.1);
            if let Some((name, me, rest)) = tile {
                let mut tokens = Default::default();
                subtree_classes(n, 0, &mut tokens);
                // The speaking ring may be drawn by the tile's frame: a few ancestors' classes too.
                for a in chain.iter().rev().take(3) {
                    tokens.extend(a.classes.iter().cloned());
                }
                tokens.extend(rest.into_iter().map(|p| format!("t:{p}")));
                out.push(Participant { key: format!("tile:{name}"), name, me, tokens });
            }
        }
        chain.push(n);
        for k in &n.kids {
            walk(k, chain, out);
        }
        chain.pop();
    }
    let mut out = vec![];
    walk(root, &mut vec![], &mut out);
    // A name seen twice (two tiles while someone shares): keep the first.
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.key.clone()));
    out
}

/// `<call>/speaking.jsonl` while a call is recorded: per participant, the tokens added and
/// removed since the last poll — `{t, k, n, me?, add, del}` (`gone: true` when the tile or
/// row disappears), `t` on the audio clock.
pub struct SpeakingLog {
    out: std::fs::File,
    bytes: u64,
    prev: std::collections::BTreeMap<String, Participant>,
}

impl SpeakingLog {
    pub fn open(dir: &std::path::Path) -> Option<Self> {
        let out = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("speaking.jsonl")).ok()?;
        Some(SpeakingLog { out, bytes: 0, prev: Default::default() })
    }

    pub fn record(&mut self, t: f64, now: Vec<Participant>) {
        use std::io::Write;
        if self.bytes > UI_LOG_MAX_BYTES {
            return;
        }
        let t = (t * 100.0).round() / 100.0;
        let mut lines = vec![];
        let mut seen = std::collections::BTreeSet::new();
        for p in now {
            seen.insert(p.key.clone());
            let before = self.prev.get(&p.key);
            if before.is_some_and(|b| b.tokens == p.tokens) {
                continue;
            }
            let empty = Default::default();
            let old = before.map(|b| &b.tokens).unwrap_or(&empty);
            let add: Vec<&String> = p.tokens.difference(old).collect();
            let del: Vec<&String> = old.difference(&p.tokens).collect();
            let mut line = serde_json::json!({ "t": t, "k": p.key, "n": p.name, "add": add, "del": del });
            if p.me {
                line["me"] = true.into();
            }
            lines.push(line.to_string());
            self.prev.insert(p.key.clone(), p);
        }
        let gone: Vec<String> = self.prev.keys().filter(|k| !seen.contains(*k)).cloned().collect();
        for k in gone {
            self.prev.remove(&k);
            lines.push(serde_json::json!({ "t": t, "k": k, "gone": true }).to_string());
        }
        for l in lines {
            if writeln!(self.out, "{l}").is_ok() {
                self.bytes += l.len() as u64 + 1;
            }
        }
    }
}

/// A saved window tree (`ui-tree-<s>.json`, `echoctl ax-dump`) as a `UiNode`, for checking the
/// participant reader against real calls.
pub fn from_dump(v: &serde_json::Value) -> UiNode {
    let str_of = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let label = ["Title", "Description", "Value", "Name"].iter().map(|k| str_of(k)).find(|t| !t.is_empty()).unwrap_or_default();
    let id = [str_of("DOMIdentifier"), str_of("AutomationId")].into_iter().find(|t| !t.is_empty()).unwrap_or_default();
    let classes = match &v["DOMClassList"] {
        serde_json::Value::Array(a) => a.iter().filter_map(|c| c.as_str().map(str::to_owned)).collect(),
        _ => str_of("ClassName").split_whitespace().map(str::to_owned).collect(),
    };
    let kids = v["children"].as_array().map(|k| k.iter().map(from_dump).collect()).unwrap_or_default();
    UiNode { role: str_of("Role"), id, classes, label, kids }
}

/// `echoctl ax-speaking`: the participants the reader finds and, live, every token that flips
/// on them (talk, pause, talk: the speaking marker is the token that follows you). With a saved
/// tree, just what it finds there.
pub fn speaking_probe(seconds: u64, tree: Option<&std::path::Path>, all: bool) -> anyhow::Result<()> {
    if let Some(path) = tree {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let roots: Vec<UiNode> = v["windows"].as_array().map(|w| w.iter().map(from_dump).collect()).unwrap_or_else(|| vec![from_dump(&v)]);
        for p in roots.iter().flat_map(participants) {
            println!("{}{}  [{}]  {} tokens", p.name, if p.me { " (me)" } else { "" }, p.key, p.tokens.len());
        }
        return Ok(());
    }
    let mut w = CallWindow::teams().ok_or_else(|| anyhow::anyhow!("Teams not running, or Accessibility not granted"))?;
    let t0 = std::time::Instant::now();
    let mut prev: std::collections::BTreeMap<String, Participant> = Default::default();
    let mut prev_all: std::collections::BTreeSet<String> = Default::default();
    while t0.elapsed() < std::time::Duration::from_secs(seconds) {
        let started = std::time::Instant::now();
        if all {
            // Every node of the call window (role, id, label, classes): what changes as people talk.
            fn flat(n: &UiNode, depth: usize, out: &mut std::collections::BTreeSet<String>) {
                out.insert(format!("{depth}|{}|{}|{}|{}", n.role, n.id, n.label.chars().take(60).collect::<String>(), n.classes.join(" ")));
                for k in &n.kids {
                    flat(k, depth + 1, out);
                }
            }
            let mut now = Default::default();
            if let Some(root) = w.tree_node() {
                flat(&root, 0, &mut now);
            }
            let t = t0.elapsed().as_secs_f64();
            if !prev_all.is_empty() {
                for x in now.difference(&prev_all) {
                    println!("[{t:5.1}s] + {x}");
                }
                for x in prev_all.difference(&now) {
                    println!("[{t:5.1}s] - {x}");
                }
            } else {
                println!("[{t:5.1}s] {} nodes", now.len());
            }
            prev_all = now;
            std::thread::sleep(std::time::Duration::from_millis(500).saturating_sub(started.elapsed()));
            continue;
        }
        for p in w.participants() {
            let t = t0.elapsed().as_secs_f64();
            match prev.get(&p.key) {
                None => println!("[{t:5.1}s] {}{}: {} tokens", p.name, if p.me { " (me)" } else { "" }, p.tokens.len()),
                Some(b) if b.tokens != p.tokens => {
                    let add: Vec<String> = p.tokens.difference(&b.tokens).map(|x| format!("+{x}")).collect();
                    let del: Vec<String> = b.tokens.difference(&p.tokens).map(|x| format!("-{x}")).collect();
                    println!("[{t:5.1}s] {}: {} {}", p.name, add.join(" "), del.join(" "));
                }
                _ => {}
            }
            prev.insert(p.key.clone(), p);
        }
        std::thread::sleep(std::time::Duration::from_millis(500).saturating_sub(started.elapsed()));
    }
    Ok(())
}

/// The call window's title right now (`echoctl` dev aid and the call's names).
pub fn call_title() -> Option<String> {
    CallWindow::teams()?.title()
}

/// Muted? from a microphone button's label: "Unmute …" ⇒ muted, "Mute …" ⇒ live.
/// Labels about notifications, speakers or other people are ignored.
pub fn mute_state_from_labels(labels: &[String]) -> Option<bool> {
    let ignore = ["notification", "speaker", "everyone", "all ", "attendee", "participant", "chat", "camera", "video"];
    for t in labels {
        if ignore.iter().any(|w| t.contains(w)) {
            continue;
        }
        if t.contains("unmute") {
            return Some(true);
        }
        if t.contains("mute") {
            return Some(false);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mute_labels_read_both_ways() {
        let l = |s: &str| vec![s.to_lowercase()];
        assert_eq!(mute_state_from_labels(&l("Unmute (⌘+Shift+M)")), Some(true));
        assert_eq!(mute_state_from_labels(&l("Mute (⌘+Shift+M)")), Some(false));
        assert_eq!(mute_state_from_labels(&l("Unmute (Ctrl+Shift+M)")), Some(true));
        assert_eq!(mute_state_from_labels(&l("Mic: Mute")), Some(false));
        assert_eq!(mute_state_from_labels(&l("Mute notifications")), None);
        assert_eq!(mute_state_from_labels(&l("Mute all attendees")), None);
        assert_eq!(mute_state_from_labels(&l("Share")), None);
    }

    #[test]
    fn who_the_call_is_with_from_the_window_title() {
        assert_eq!(call_name("Jane Doe | Contoso | jane@contoso.com | Microsoft Teams").as_deref(), Some("Jane Doe"));
        assert_eq!(call_name("Call with Jane Doe | Microsoft Teams").as_deref(), Some("Jane Doe"));
        assert_eq!(call_name("Weekly sync | Microsoft Teams").as_deref(), Some("Weekly sync"));
        assert_eq!(call_name("Microsoft Teams"), None);
        assert_eq!(call_name("Meeting | Microsoft Teams"), None);
        assert!(is_person_name("Jane Doe") && is_person_name("Mary-Ann O'Neil"));
        assert!(!is_person_name("Weekly sync") && !is_person_name("Jane") && !is_person_name("Q3 Planning Review"));
    }

    #[test]
    fn participants_from_tiles_and_the_roster() {
        let node = |role: &str, id: &str, label: &str, classes: &[&str], kids: Vec<UiNode>| UiNode {
            role: role.into(), id: id.into(), label: label.into(), classes: classes.iter().map(|c| c.to_string()).collect(), kids,
        };
        let tree = node("AXGroup", "", "", &["stage"], vec![
            node("AXGroup", "", "", &["frame", "lit"], vec![node("AXMenuItem", "", "Daniel Osei, video is on, muted, Context menu is available", &["tile"], vec![
                node("AXGroup", "", "", &["ring"], vec![]),
            ])]),
            node("AXMenuItem", "", "Myself video, Jane Doe, Unmuted, Has context menu", &["tile"], vec![]),
            node("AXMenuItem", "", "Osei, Daniel (Contoso), muted", &["tile"], vec![]),
            node("AXMenuItem", "", "Share", &[], vec![]),
            node("AXMenuItem", "", "Maren Ortiz", &["compact"], vec![]),
            node("AXImage", "", "Myself video, Jane Doe, Unmuted", &[], vec![]),
            node("AXImage", "", "Profile picture of Lee Park", &[], vec![]),
            node("AXRow", "", "Priya Nair, Unmuted", &[], vec![node("AXGroup", "calling-roster-item-8:orgid:abc", "", &["item"], vec![
                node("AXGroup", "roster-avatar-img-8:orgid:abc", "", &["___speaking"], vec![]),
            ])]),
        ]);
        let p = participants(&tree);
        let names: Vec<(&str, bool)> = p.iter().map(|x| (x.name.as_str(), x.me)).collect();
        // The second "Jane Doe" (her preview as an image) shares her tile's key and is dropped.
        assert_eq!(names, [("Daniel Osei", false), ("Jane Doe", true), ("Osei, Daniel (Contoso)", false), ("Maren Ortiz", false), ("Priya Nair", false)]);
        assert!(p[0].tokens.contains("ring") && p[0].tokens.contains("lit") && p[0].tokens.contains("t:muted"));
        assert_eq!(p[4].key, "roster:8:orgid:abc");
        assert!(p[4].tokens.contains("___speaking") && p[4].tokens.contains("t:unmuted"));
    }

    #[test]
    fn sharing_from_the_call_window() {
        let read = |nodes: &[(&str, &str)]| {
            let mut s = ShareSeen::default();
            nodes.iter().for_each(|(id, label)| s.see(id, label));
            s.sharing()
        };
        assert_eq!(read(&[(MIC_BUTTON_ID, "Mute mic"), (SHARE_BUTTON_ID, "Share")]), Sharing::Nobody);
        assert_eq!(read(&[(POPOUT_ID, "Pop out")]), Sharing::Them(None)); // any language
        assert_eq!(read(&[("", "Content shared by Jane Doe "), (POPOUT_ID, "")]), Sharing::Them(Some("Jane Doe".into())));
        assert_eq!(read(&[(SHARE_BUTTON_ID, "Stop sharing (⌘+Shift+E)")]), Sharing::Me);
    }
}
