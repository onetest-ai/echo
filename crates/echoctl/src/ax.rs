//! macOS Accessibility (AX) reading of the call app's window — the basis for naming remote
//! speakers (Teams' participant tiles expose names and
//! a speaking state through accessibility; text only, no screenshots). During every recording
//! `UiRecorder` logs the window's text changes into the call folder, which is what the speaker
//! reader is built and tuned from; `MuteWatch` follows the mute button. `echoctl ax-dump`
//! remains a dev tool.
//!
//! Requires the Accessibility permission for Echo (or the terminal running the CLI).

use std::ffi::c_void;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use objc2::rc::Retained;
use objc2_foundation::NSString;
use serde_json::{json, Map, Value};

use crate::paths;

type CFTypeRef = *const c_void;
type AXUIElementRef = *const c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(e: AXUIElementRef, attr: CFTypeRef, out: *mut CFTypeRef) -> i32;
    fn AXUIElementSetAttributeValue(e: AXUIElementRef, attr: CFTypeRef, value: CFTypeRef) -> i32;
    fn AXUIElementGetTypeID() -> usize;
    fn AXIsProcessTrusted() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFArrayGetCount(a: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(a: CFTypeRef, i: isize) -> CFTypeRef;
    fn CFBooleanGetValue(b: CFTypeRef) -> bool;
    fn CFNumberGetValue(n: CFTypeRef, kind: isize, out: *mut c_void) -> bool;
    fn CFRelease(cf: CFTypeRef);
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    static kCFBooleanTrue: CFTypeRef;
}
const K_CF_NUMBER_DOUBLE: isize = 13;

/// An owned AX element (released on drop).
pub struct Element(AXUIElementRef);

impl Drop for Element {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

fn cfstr(s: &str) -> Retained<NSString> {
    NSString::from_str(s) // toll-free bridged to CFString
}

/// An AX attribute value converted to JSON (strings, bools, numbers, string lists).
enum Attr {
    Json(Value),
    Elements(Vec<Element>),
}

impl Element {
    pub fn application(pid: i32) -> Option<Self> {
        let e = unsafe { AXUIElementCreateApplication(pid) };
        (!e.is_null()).then_some(Element(e))
    }

    fn raw(&self, name: &str) -> Option<CFTypeRef> {
        let attr = cfstr(name);
        let mut out: CFTypeRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(self.0, Retained::as_ptr(&attr).cast(), &mut out) };
        (err == 0 && !out.is_null()).then_some(out)
    }

    fn attr(&self, name: &str) -> Option<Attr> {
        let v = self.raw(name)?;
        let out = unsafe { convert(v) };
        unsafe { CFRelease(v) };
        out
    }

    pub fn string(&self, name: &str) -> Option<String> {
        match self.attr(name)? {
            Attr::Json(Value::String(s)) if !s.is_empty() => Some(s),
            _ => None,
        }
    }

    pub fn children(&self, name: &str) -> Vec<Element> {
        match self.attr(name) {
            Some(Attr::Elements(v)) => v,
            _ => vec![],
        }
    }

    /// Ask Chromium/WebKit-based apps to expose their web content to accessibility.
    pub fn enable_web_accessibility(&self) {
        for name in ["AXManualAccessibility", "AXEnhancedUserInterface"] {
            let attr = cfstr(name);
            unsafe { AXUIElementSetAttributeValue(self.0, Retained::as_ptr(&attr).cast(), kCFBooleanTrue) };
        }
    }
}

unsafe fn cf_to_string(v: CFTypeRef) -> String {
    // CFString is toll-free bridged to NSString: `retain` +1 now, the Retained releases it.
    Retained::retain(v as *mut NSString).map(|s| s.to_string()).unwrap_or_default()
}

unsafe fn convert(v: CFTypeRef) -> Option<Attr> {
    let t = CFGetTypeID(v);
    if t == CFStringGetTypeID() {
        return Some(Attr::Json(Value::String(cf_to_string(v))));
    }
    if t == CFBooleanGetTypeID() {
        return Some(Attr::Json(Value::Bool(CFBooleanGetValue(v))));
    }
    if t == CFNumberGetTypeID() {
        let mut d = 0f64;
        CFNumberGetValue(v, K_CF_NUMBER_DOUBLE, (&mut d as *mut f64).cast());
        return Some(Attr::Json(json!(d)));
    }
    if t == CFArrayGetTypeID() {
        let n = CFArrayGetCount(v);
        let mut els = vec![];
        let mut strs = vec![];
        for i in 0..n {
            let item = CFArrayGetValueAtIndex(v, i);
            let it = CFGetTypeID(item);
            if it == AXUIElementGetTypeID() {
                els.push(Element(CFRetain(item)));
            } else if it == CFStringGetTypeID() {
                strs.push(Value::String(cf_to_string(item)));
            }
        }
        return Some(if !els.is_empty() { Attr::Elements(els) } else { Attr::Json(Value::Array(strs)) });
    }
    None
}

const FIELDS: &[&str] = &[
    "AXRole", "AXSubrole", "AXRoleDescription", "AXTitle", "AXDescription", "AXValue", "AXHelp",
    "AXIdentifier", "AXDOMIdentifier", "AXDOMClassList", "AXSelected", "AXFocused",
];
const MAX_DEPTH: usize = 60;
const MAX_NODES: usize = 25_000;

fn dump_node(e: &Element, depth: usize, count: &mut usize) -> Value {
    *count += 1;
    let mut m = Map::new();
    for f in FIELDS {
        if let Some(Attr::Json(v)) = e.attr(f) {
            let empty = matches!(&v, Value::String(s) if s.is_empty()) || matches!(&v, Value::Array(a) if a.is_empty());
            if !empty {
                m.insert(f.trim_start_matches("AX").to_string(), v);
            }
        }
    }
    if depth < MAX_DEPTH && *count < MAX_NODES {
        let kids: Vec<Value> = e.children("AXChildren").iter().map(|c| dump_node(c, depth + 1, count)).collect();
        if !kids.is_empty() {
            m.insert("children".into(), Value::Array(kids));
        }
    }
    Value::Object(m)
}

/// The main (UI) process of the target app, by its executable.
pub fn app_pid(executable_suffix: &str) -> Option<i32> {
    let out = Command::new("pgrep").args(["-f", executable_suffix]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().find_map(|l| l.trim().parse().ok())
}

pub const TEAMS_EXECUTABLE: &str = "Microsoft Teams.app/Contents/MacOS/MSTeams";

/// `echoctl ax-dump`: snapshot Teams' window tree every second for `seconds` seconds.
pub fn dump(seconds: u64) -> Result<()> {
    if !unsafe { AXIsProcessTrusted() } {
        bail!("Accessibility is not granted to this terminal — run `echoctl doctor --open accessibility`");
    }
    let pid = app_pid(TEAMS_EXECUTABLE).ok_or_else(|| anyhow!("Microsoft Teams is not running"))?;
    let app = Element::application(pid).ok_or_else(|| anyhow!("cannot reach Teams (pid {pid})"))?;
    app.enable_web_accessibility();
    std::thread::sleep(Duration::from_millis(500)); // let the web tree appear

    let dir: PathBuf = paths::state_dir().join("ax").join(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string());
    std::fs::create_dir_all(&dir)?;
    let t0 = Instant::now();
    let mut n = 0;
    while t0.elapsed() < Duration::from_secs(seconds) {
        let started = Instant::now();
        let mut count = 0;
        let windows: Vec<Value> = app.children("AXWindows").iter().map(|w| dump_node(w, 0, &mut count)).collect();
        let snap = json!({ "t": t0.elapsed().as_secs_f64(), "pid": pid, "nodes": count, "windows": windows });
        std::fs::write(dir.join(format!("{n:03}.json")), serde_json::to_vec(&snap)?)?;
        eprintln!("  snapshot {n}: {count} nodes in {} ms", started.elapsed().as_millis());
        n += 1;
        std::thread::sleep(Duration::from_millis(1000).saturating_sub(started.elapsed()));
    }
    println!("{}", dir.display());
    Ok(())
}


// ---------------------------------------------------------------------------------------------
// Teams mute state: find the call window's microphone button and read its label.

const BUTTON_ROLES: &[&str] = &["AXButton", "AXCheckBox", "AXToggle", "AXSwitch", "AXMenuButton"];

fn texts(e: &Element) -> Vec<String> {
    ["AXTitle", "AXDescription", "AXHelp", "AXValue"]
        .iter()
        .filter_map(|a| e.string(a))
        .map(|t| t.to_lowercase())
        .collect()
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

fn find_mute_button(e: Element, depth: usize) -> Option<Element> {
    let role = e.string("AXRole").unwrap_or_default();
    if BUTTON_ROLES.contains(&role.as_str()) && mute_state_from_labels(&texts(&e)).is_some() {
        return Some(e);
    }
    if depth >= MAX_DEPTH {
        return None;
    }
    e.children("AXChildren").into_iter().find_map(|c| find_mute_button(c, depth + 1))
}

/// Polls the call app's mute button. Finds it once, then only re-reads its label (cheap);
/// searches again if it disappears (window closed, layout changed).
pub struct MuteWatch {
    app: Element,
    button: Option<Element>,
    last_search: Option<Instant>,
}

impl MuteWatch {
    pub fn teams() -> Option<Self> {
        if !unsafe { AXIsProcessTrusted() } {
            return None;
        }
        let app = Element::application(app_pid(TEAMS_EXECUTABLE)?)?;
        app.enable_web_accessibility();
        Some(MuteWatch { app, button: None, last_search: None })
    }

    /// Some(true) = muted in the app, Some(false) = live, None = can't tell.
    pub fn poll(&mut self) -> Option<bool> {
        if let Some(b) = &self.button {
            match mute_state_from_labels(&texts(b)) {
                Some(state) => return Some(state),
                None => self.button = None, // gone or relabelled: search again
            }
        }
        if self.last_search.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
            return None;
        }
        self.last_search = Some(Instant::now());
        self.button = self.app.children("AXWindows").into_iter().find_map(|w| find_mute_button(w, 0));
        self.button.as_ref().and_then(|b| mute_state_from_labels(&texts(b)))
    }
}

// ── per-call UI timeline (for speaker names) ───────────────────────────────────────────────

/// One line per text-bearing node: "depth|role|subrole|title|description|value". Diffing these
/// between polls shows what changes when someone speaks (a tile's label, a "speaking" state).
fn ui_lines(e: &Element, depth: usize, out: &mut Vec<String>, budget: &mut usize) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    let get = |a: &str| e.string(a).map(|t| t.chars().take(160).collect::<String>().replace(['|', '\n'], " ")).unwrap_or_default();
    let (title, desc, value) = (get("AXTitle"), get("AXDescription"), get("AXValue"));
    if !(title.is_empty() && desc.is_empty() && value.is_empty()) {
        out.push(format!("{depth}|{}|{}|{title}|{desc}|{value}", get("AXRole"), get("AXSubrole")));
    }
    if depth < MAX_DEPTH {
        for c in e.children("AXChildren") {
            ui_lines(&c, depth + 1, out, budget);
        }
    }
}

const UI_LOG_MAX_BYTES: u64 = 40_000_000;
/// Full window trees saved at these offsets into the call (structure for building the reader).
const UI_TREES_AT: &[u64] = &[10, 120, 600];

/// Records Teams' window text while a call is recorded: `<call>/ui.jsonl` holds
/// `{t, add, del}` lines (t = seconds since the recording started, aligned with the audio),
/// `<call>/ui-tree-<s>.json` a few full trees. Text only — no screenshots.
pub struct UiRecorder {
    app: Element,
    dir: PathBuf,
    t0: Instant,
    out: std::fs::File,
    bytes: u64,
    prev: std::collections::BTreeSet<String>,
    trees_done: usize,
}

impl UiRecorder {
    pub fn teams(dir: &std::path::Path, t0: Instant) -> Option<Self> {
        if !unsafe { AXIsProcessTrusted() } {
            return None;
        }
        let app = Element::application(app_pid(TEAMS_EXECUTABLE)?)?;
        app.enable_web_accessibility();
        let out = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("ui.jsonl")).ok()?;
        Some(UiRecorder { app, dir: dir.to_path_buf(), t0, out, bytes: 0, prev: Default::default(), trees_done: 0 })
    }

    /// Read the tree once and append what changed. Returns false once the log is full.
    pub fn poll(&mut self) -> bool {
        use std::io::Write;
        if self.bytes > UI_LOG_MAX_BYTES {
            return false;
        }
        let t = self.t0.elapsed().as_secs_f64();
        let windows = self.app.children("AXWindows");
        let mut lines = vec![];
        let mut budget = MAX_NODES;
        for w in &windows {
            ui_lines(w, 0, &mut lines, &mut budget);
        }
        let now: std::collections::BTreeSet<String> = lines.into_iter().collect();
        let add: Vec<&String> = now.difference(&self.prev).collect();
        let del: Vec<&String> = self.prev.difference(&now).collect();
        if !add.is_empty() || !del.is_empty() {
            let line = json!({ "t": (t * 10.0).round() / 10.0, "add": add, "del": del }).to_string();
            if writeln!(self.out, "{line}").is_ok() {
                self.bytes += line.len() as u64 + 1;
            }
        }
        self.prev = now;
        if let Some(&at) = UI_TREES_AT.get(self.trees_done) {
            if t >= at as f64 {
                self.trees_done += 1;
                let mut count = 0;
                let tree: Vec<Value> = windows.iter().map(|w| dump_node(w, 0, &mut count)).collect();
                let snap = json!({ "t": t, "nodes": count, "windows": tree });
                let _ = std::fs::write(self.dir.join(format!("ui-tree-{at}s.json")), snap.to_string());
            }
        }
        true
    }
}

/// `echoctl ax-mic`: show what the mute detector sees (run during a Teams call).
pub fn mic_probe(seconds: u64) -> Result<()> {
    let mut w = MuteWatch::teams().ok_or_else(|| anyhow!("Teams not running, or Accessibility not granted"))?;
    let t0 = Instant::now();
    let mut last: Option<Option<bool>> = None;
    while t0.elapsed() < Duration::from_secs(seconds) {
        let s = w.poll();
        if last != Some(s) {
            let label = w.button.as_ref().map(texts).unwrap_or_default().join(" | ");
            println!(
                "[{:5.1}s] {}  {}",
                t0.elapsed().as_secs_f64(),
                match s { Some(true) => "MUTED", Some(false) => "live ", None => "?    " },
                if label.is_empty() { "(no microphone button found — is a call window open?)".into() } else { label }
            );
            last = Some(s);
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mute_labels_read_both_ways() {
        let l = |s: &str| vec![s.to_lowercase()];
        assert_eq!(mute_state_from_labels(&l("Unmute (⌘+Shift+M)")), Some(true));
        assert_eq!(mute_state_from_labels(&l("Mute (⌘+Shift+M)")), Some(false));
        assert_eq!(mute_state_from_labels(&l("Mic: Mute")), Some(false));
        assert_eq!(mute_state_from_labels(&l("Mute notifications")), None);
        assert_eq!(mute_state_from_labels(&l("Mute all attendees")), None);
        assert_eq!(mute_state_from_labels(&l("Share")), None);
    }
}
