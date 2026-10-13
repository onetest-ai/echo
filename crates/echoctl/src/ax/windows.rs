//! Windows UI Automation reading of the call app's windows: the Teams windows are the top-level
//! windows owned by its process tree; their web content (WebView2) is exposed to UIA once a
//! client asks for it. Each read fetches a window's whole subtree in one cross-process call
//! (a cache request), then walks the cached copy. No permission is needed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};
use windows::core::BSTR;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible};

use super::{mute_state_from_labels, ShareSeen, Sharing, MIC_BUTTON_ID, UI_LOG_MAX_BYTES, UI_TREES_AT};
use crate::apps::TargetApp;
use crate::paths;

const MAX_DEPTH: usize = 60;
const MAX_NODES: usize = 25_000;

/// COM (MTA) for this thread while a reader lives; tolerates a thread already initialised.
struct Com(bool);

impl Com {
    fn init() -> Self {
        Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// One node of a cached subtree.
struct Node(IUIAutomationElement);

impl Node {
    fn name(&self) -> String {
        unsafe { self.0.CachedName() }.map(|b| b.to_string()).unwrap_or_default()
    }

    fn control_type(&self) -> i32 {
        unsafe { self.0.CachedControlType() }.map(|t| t.0).unwrap_or(0)
    }

    fn role(&self) -> String {
        control_type_name(self.control_type())
    }

    fn automation_id(&self) -> String {
        unsafe { self.0.CachedAutomationId() }.map(|b| b.to_string()).unwrap_or_default()
    }

    /// Chromium (WebView2) reports the element's HTML class attribute as its ClassName.
    fn class_name(&self) -> String {
        unsafe { self.0.CachedClassName() }.map(|b| b.to_string()).unwrap_or_default()
    }

    fn help(&self) -> String {
        unsafe { self.0.CachedHelpText() }.map(|b| b.to_string()).unwrap_or_default()
    }

    fn value(&self) -> String {
        unsafe { self.0.GetCachedPropertyValue(UIA_ValueValuePropertyId) }
            .ok()
            .and_then(|v: VARIANT| BSTR::try_from(&v).ok())
            .map(|b| b.to_string())
            .unwrap_or_default()
    }

    fn children(&self) -> Vec<Node> {
        let Ok(arr) = (unsafe { self.0.GetCachedChildren() }) else { return vec![] };
        let n = unsafe { arr.Length() }.unwrap_or(0);
        (0..n).filter_map(|i| unsafe { arr.GetElement(i) }.ok().map(Node)).collect()
    }
}

fn control_type_name(id: i32) -> String {
    let name = match id {
        50000 => "Button",
        50002 => "CheckBox",
        50003 => "ComboBox",
        50004 => "Edit",
        50005 => "Hyperlink",
        50006 => "Image",
        50007 => "ListItem",
        50008 => "List",
        50009 => "Menu",
        50010 => "MenuBar",
        50011 => "MenuItem",
        50013 => "RadioButton",
        50018 => "Tab",
        50019 => "TabItem",
        50020 => "Text",
        50021 => "ToolBar",
        50022 => "ToolTip",
        50023 => "Tree",
        50024 => "TreeItem",
        50025 => "Custom",
        50026 => "Group",
        50029 => "DataItem",
        50030 => "Document",
        50031 => "SplitButton",
        50032 => "Window",
        50033 => "Pane",
        50036 => "Table",
        50037 => "TitleBar",
        _ => return id.to_string(),
    };
    name.into()
}

/// UIA client + the call app's top-level windows.
struct Reader {
    uia: IUIAutomation,
    cache: IUIAutomationCacheRequest,
    app: TargetApp,
    /// Last: dropped after the interfaces above.
    _com: Com,
}

impl Reader {
    fn new(app: TargetApp) -> Option<Self> {
        let com = Com::init();
        unsafe {
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let cache = uia.CreateCacheRequest().ok()?;
            for p in [
                UIA_NamePropertyId,
                UIA_ControlTypePropertyId,
                UIA_AutomationIdPropertyId,
                UIA_HelpTextPropertyId,
                UIA_ClassNamePropertyId,
                UIA_ValueValuePropertyId,
                UIA_ProcessIdPropertyId,
            ] {
                cache.AddProperty(p).ok()?;
            }
            cache.SetTreeScope(TreeScope_Subtree).ok()?;
            Some(Reader { uia, cache, app, _com: com })
        }
    }

    /// The app's visible top-level windows (live elements). Found with EnumWindows: asking UIA
    /// for the desktop's children waits on every hung window (seconds).
    fn windows(&self) -> Vec<IUIAutomationElement> {
        let pids = crate::audio::windows::app_tree(self.app).unwrap_or_default();
        if pids.is_empty() {
            return vec![];
        }
        top_level_windows(&pids).into_iter().filter_map(|h| unsafe { self.uia.ElementFromHandle(h) }.ok()).collect()
    }

    /// Each app window with its subtree cached.
    fn trees(&self) -> Vec<Node> {
        self.windows().iter().filter_map(|w| unsafe { w.BuildUpdatedCache(&self.cache) }.ok().map(Node)).collect()
    }

    /// The call window: the app window holding the microphone button (the main window shows
    /// chats, which must never end up in a call's frames).
    fn meeting_window(&self) -> Option<HWND> {
        let pids = crate::audio::windows::app_tree(self.app).ok()?;
        top_level_windows(&pids).into_iter().find(|&h| {
            let tree = unsafe { self.uia.ElementFromHandle(h).and_then(|w| w.BuildUpdatedCache(&self.cache)) };
            tree.is_ok_and(|t| find_mute_button(Node(t), 0).is_some())
        })
    }
}

/// Teams' call window, found once and kept: a call's UI text and frames come from it alone
/// (the main window shows chats). Finding it reads every Teams window's tree (~60 ms), so it is
/// looked for again only every few seconds, or when the window closes.
pub struct CallWindow {
    hwnd: Option<HWND>,
    looked_at: Option<Instant>,
    /// Last: COM objects are released before it uninitialises COM.
    reader: Reader,
}

impl CallWindow {
    pub fn teams() -> Option<Self> {
        Some(CallWindow { hwnd: None, looked_at: None, reader: Reader::new(TargetApp::Teams)? })
    }

    /// The call window, if Teams has one open.
    pub fn hwnd(&mut self) -> Option<HWND> {
        let every = if self.hwnd.is_some() { Duration::from_secs(10) } else { Duration::from_secs(3) };
        let closed = self.hwnd.is_some_and(|h| !unsafe { IsWindow(Some(h)) }.as_bool());
        if closed || self.looked_at.is_none_or(|t| t.elapsed() >= every) {
            self.hwnd = self.reader.meeting_window();
            self.looked_at = Some(Instant::now());
        }
        self.hwnd
    }

    /// The call window's title.
    pub fn title(&mut self) -> Option<String> {
        let h = self.hwnd()?;
        let mut buf = [0u16; 512];
        let n = unsafe { GetWindowTextW(h, &mut buf) };
        (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
    }

    /// The participants the call window shows right now (`echoctl ax-speaking`).
    pub fn participants(&mut self) -> Vec<super::Participant> {
        self.tree_node().map(|t| super::participants(&t)).unwrap_or_default()
    }

    /// The call window as a light tree.
    pub fn tree_node(&mut self) -> Option<super::UiNode> {
        let tree = self.tree()?;
        let (mut lines, mut budget) = (vec![], MAX_NODES);
        Some(ui_lines(&tree, 0, &mut lines, &mut budget))
    }

    /// The call window's subtree (cached), if there is one.
    fn tree(&mut self) -> Option<Node> {
        let h = self.hwnd()?;
        unsafe { self.reader.uia.ElementFromHandle(h).and_then(|w| w.BuildUpdatedCache(&self.reader.cache)) }.ok().map(Node)
    }

    /// Who is presenting in the call, from the call window (measured live with Teams 26225).
    /// Someone else's share: the stage gets a pop-out button (AutomationId
    /// `popout-content-button`, whatever Teams' language; absent while you present) and, in
    /// English, a "Content shared by <name>" label — the only source of the name. Your own
    /// share: the `share-button` reads "Stop sharing" (English only).
    pub fn sharing(&mut self) -> Sharing {
        let Some(tree) = self.tree() else { return Sharing::Nobody };
        fn walk(n: &Node, depth: usize, seen: &mut ShareSeen) {
            seen.see(&n.automation_id(), &n.name());
            if depth < MAX_DEPTH {
                for c in n.children() {
                    walk(&c, depth + 1, seen);
                }
            }
        }
        let mut seen = ShareSeen::default();
        walk(&tree, 0, &mut seen);
        seen.sharing()
    }
}

/// Visible top-level windows owned by these processes.
fn top_level_windows(pids: &[u32]) -> Vec<HWND> {
    unsafe extern "system" fn each(h: HWND, acc: LPARAM) -> BOOL {
        let acc = &mut *(acc.0 as *mut (Vec<u32>, Vec<HWND>));
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        if acc.0.contains(&pid) && IsWindowVisible(h).as_bool() {
            acc.1.push(h);
        }
        true.into()
    }
    let mut acc: (Vec<u32>, Vec<HWND>) = (pids.to_vec(), vec![]);
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut acc as *mut _ as isize));
    }
    acc.1
}

fn dump_node(n: &Node, depth: usize, count: &mut usize) -> Value {
    *count += 1;
    let mut m = Map::new();
    for (k, v) in [("Role", n.role()), ("Name", n.name()), ("AutomationId", n.automation_id()), ("Help", n.help()), ("Value", n.value())] {
        if !v.is_empty() {
            m.insert(k.into(), Value::String(v));
        }
    }
    if depth < MAX_DEPTH && *count < MAX_NODES {
        let kids: Vec<Value> = n.children().iter().map(|c| dump_node(c, depth + 1, count)).collect();
        if !kids.is_empty() {
            m.insert("children".into(), Value::Array(kids));
        }
    }
    Value::Object(m)
}

/// `echoctl ax-dump`: snapshot Teams' window trees every second for `seconds` seconds.
pub fn dump(seconds: u64) -> Result<()> {
    let r = Reader::new(TargetApp::Teams).ok_or_else(|| anyhow!("UI Automation is not available"))?;
    if r.windows().is_empty() {
        return Err(anyhow!("Microsoft Teams is not running (no Teams windows)"));
    }
    let dir: PathBuf = paths::state_dir().join("ax").join(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string());
    std::fs::create_dir_all(&dir)?;
    let t0 = Instant::now();
    let mut n = 0;
    while t0.elapsed() < Duration::from_secs(seconds) {
        let started = Instant::now();
        let mut count = 0;
        let windows: Vec<Value> = r.trees().iter().map(|w| dump_node(w, 0, &mut count)).collect();
        let snap = json!({ "t": t0.elapsed().as_secs_f64(), "nodes": count, "windows": windows });
        std::fs::write(dir.join(format!("{n:03}.json")), serde_json::to_vec(&snap)?)?;
        eprintln!("  snapshot {n}: {count} nodes in {} ms", started.elapsed().as_millis());
        n += 1;
        std::thread::sleep(Duration::from_millis(1000).saturating_sub(started.elapsed()));
    }
    println!("{}", dir.display());
    Ok(())
}

// ── Teams mute state ───────────────────────────────────────────────────────────────────────

/// Teams' own microphone button in the call window (`MIC_BUTTON_ID`, labelled "Mute mic" /
/// "Unmute mic"). Only this one: a label match would also take a roster's "Mute <name>" (an
/// organizer's view).
fn find_mute_button(n: Node, depth: usize) -> Option<Node> {
    if n.automation_id() == MIC_BUTTON_ID {
        return Some(n);
    }
    if depth >= MAX_DEPTH {
        return None;
    }
    n.children().into_iter().find_map(|c| find_mute_button(c, depth + 1))
}

/// Polls the call app's mute button. Finds it once (a cached search), then only re-reads its
/// live name (cheap); searches again if it disappears (window closed, layout changed).
pub struct MuteWatch {
    button: Option<IUIAutomationElement>,
    last_search: Option<Instant>,
    /// Last: COM elements above must be released before it uninitialises COM.
    reader: Reader,
}

fn live_texts(e: &IUIAutomationElement) -> Vec<String> {
    let name = unsafe { e.CurrentName() }.map(|b| b.to_string()).unwrap_or_default();
    let help = unsafe { e.CurrentHelpText() }.map(|b| b.to_string()).unwrap_or_default();
    [name, help].into_iter().filter(|t| !t.is_empty()).map(|t| t.to_lowercase()).collect()
}

impl MuteWatch {
    pub fn teams() -> Option<Self> {
        let reader = Reader::new(TargetApp::Teams)?;
        if reader.windows().is_empty() {
            return None;
        }
        Some(MuteWatch { button: None, last_search: None, reader })
    }

    /// Some(true) = muted in the app, Some(false) = live, None = can't tell.
    pub fn poll(&mut self) -> Option<bool> {
        if let Some(b) = &self.button {
            match mute_state_from_labels(&live_texts(b)) {
                Some(state) => return Some(state),
                None => self.button = None, // gone or relabelled: search again
            }
        }
        if self.last_search.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
            return None;
        }
        self.last_search = Some(Instant::now());
        self.button = self.reader.trees().into_iter().find_map(|w| find_mute_button(w, 0)).map(|n| n.0);
        self.button.as_ref().and_then(|b| mute_state_from_labels(&live_texts(b)))
    }
}

/// `echoctl ax-mic`: show what the mute detector sees (run during a Teams call).
pub fn mic_probe(seconds: u64) -> Result<()> {
    let mut w = MuteWatch::teams().ok_or_else(|| anyhow!("Teams is not running (no Teams windows)"))?;
    let t0 = Instant::now();
    let mut last: Option<Option<bool>> = None;
    while t0.elapsed() < Duration::from_secs(seconds) {
        let s = w.poll();
        if last != Some(s) {
            let label = w.button.as_ref().map(live_texts).unwrap_or_default().join(" | ");
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

// ── per-call UI timeline (for speaker names) ───────────────────────────────────────────────

/// One line per text-bearing node: "depth|role|automation id|name|help|value" — the same
/// shape as macOS's "depth|role|subrole|title|description|value". The same walk returns a light
/// copy of the tree (with class names) for the participant reader.
fn ui_lines(n: &Node, depth: usize, out: &mut Vec<String>, budget: &mut usize) -> super::UiNode {
    let mut node = super::UiNode::default();
    if *budget == 0 {
        return node;
    }
    *budget -= 1;
    let clean = |s: &str| s.chars().take(160).collect::<String>().replace(['|', '\n'], " ");
    let (name, help, value, id) = (n.name(), n.help(), n.value(), n.automation_id());
    let role = n.role();
    if !(name.is_empty() && help.is_empty() && value.is_empty()) {
        out.push(format!("{depth}|{role}|{}|{}|{}|{}", clean(&id), clean(&name), clean(&help), clean(&value)));
    }
    node.label = name.chars().take(300).collect();
    node.classes = n.class_name().split_whitespace().map(str::to_owned).collect();
    node.id = id;
    node.role = role;
    if depth < MAX_DEPTH {
        for c in n.children() {
            node.kids.push(ui_lines(&c, depth + 1, out, budget));
        }
    }
    node
}

/// Records the text of Teams' call window (not the main window, which shows chats) while a
/// call is recorded: `<call>/ui.jsonl` holds `{t, add, del}` lines (t = seconds since the
/// recording started, aligned with the audio), `<call>/ui-tree-<s>.json` a few full trees.
/// Text only — no screenshots.
pub struct UiRecorder {
    speaking: Option<super::SpeakingLog>,
    dir: PathBuf,
    t0: Instant,
    out: std::fs::File,
    bytes: u64,
    prev: BTreeSet<String>,
    trees_done: usize,
    window: CallWindow,
}

impl UiRecorder {
    pub fn teams(dir: &Path, t0: Instant) -> Option<Self> {
        let window = CallWindow::teams()?;
        if window.reader.windows().is_empty() {
            return None;
        }
        let out = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("ui.jsonl")).ok()?;
        let speaking = super::SpeakingLog::open(dir);
        Some(UiRecorder { speaking, dir: dir.to_path_buf(), t0, out, bytes: 0, prev: Default::default(), trees_done: 0, window })
    }

    /// Read the call window once and append what changed. Returns false once the log is full.
    pub fn poll(&mut self) -> bool {
        use std::io::Write;
        if self.bytes > UI_LOG_MAX_BYTES {
            return false;
        }
        let t = self.t0.elapsed().as_secs_f64();
        let windows: Vec<Node> = self.window.tree().into_iter().collect();
        let mut lines = vec![];
        let mut budget = MAX_NODES;
        let mut people = vec![];
        for w in &windows {
            people.extend(super::participants(&ui_lines(w, 0, &mut lines, &mut budget)));
        }
        if let (Some(log), false) = (self.speaking.as_mut(), windows.is_empty()) {
            log.record(t, people);
        }
        let now: BTreeSet<String> = lines.into_iter().collect();
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
