//! Menu-bar (macOS) / notification-area (Windows) icon of the echoctl app.
//!
//! The app's whole UI: status, Record / Stop, upcoming meetings, recent calls, settings,
//! permissions and Quit. Clicks call the same `arm` / `stop` as the CLI and Claude Code's
//! skills, so every client sees the same events.
//!
//! macOS needs the event loop on the main thread, so the watcher loop runs on a worker thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Local, Utc};

use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::apps::TargetApp;
use crate::config::Config;
use crate::meeting::MeetingContext;
use crate::watch::{self, UiState};
use crate::{app, models, paths, permissions};

enum UiEvent {
    State(UiState),
    Menu(MenuId),
    /// A message from the Echo window's page.
    Ipc(String),
    Quit,
}

const SIZE: u32 = 36;

/// Recording: warm orange; armed: green; idle: black as a template (follows the menu bar),
/// white on a dark Windows taskbar.
const ORANGE: [u8; 3] = [0xD9, 0x77, 0x57];
const BLACK: [u8; 3] = [0, 0, 0];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const GREEN: [u8; 3] = [52, 199, 89];

/// The octopus mark (`mark.rs`) in `color`, SIZE×SIZE.
fn logo(color: [u8; 3]) -> Icon {
    Icon::from_rgba(crate::mark::mark_rgba(SIZE, color), SIZE, SIZE).expect("valid icon")
}

/// The app icon (Dock-style, for Echo.app), RGBA `size`×`size`.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    crate::mark::app_icon_rgba(size)
}

/// Minimal PNG encoder (stored deflate blocks) — enough for build-time icons, no dependency.
pub fn png(rgba: &[u8], size: u32) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks(size as usize * 4) {
        raw.push(0); // filter: none
        raw.extend(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
        z.extend((b.len() as u16).to_le_bytes());
        z.extend((!(b.len() as u16)).to_le_bytes());
        z.extend(*b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = vec![];
    ihdr.extend(size.to_be_bytes());
    ihdr.extend(size.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]); // 8-bit RGBA
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

const ICONSET: [(&str, u32); 10] = [
    ("icon_16x16", 16), ("icon_16x16@2x", 32), ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256), ("icon_256x256", 256), ("icon_256x256@2x", 512),
    ("icon_512x512", 512), ("icon_512x512@2x", 1024),
];

/// `echoctl dev-icon <file>.ico [--from logo.png]`: the Windows app icon (embedded in
/// echoctl.exe by build.rs) at 16…256 px, each a PNG entry (Windows Vista+ reads those). With
/// `--from` (the onetest logo) it is the macOS icon's artwork — the octopus on the logo's purple
/// rounded square — without Apple's margin, so it fills the tile like other Windows icons;
/// otherwise the drawn mark.
fn write_ico(path: &std::path::Path, from: Option<&std::path::Path>) -> anyhow::Result<()> {
    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];
    use image::ImageEncoder;
    let art = match from {
        Some(src) => {
            let (_, art) = logo_icon(&image::open(src)?.to_rgba8(), 1024, true);
            let m = (1024 - 824) / 2; // logo_icon draws macOS's 824/1024 body
            Some(image::imageops::crop_imm(&art, m, m, 824, 824).to_image())
        }
        None => None,
    };
    let pngs = SIZES
        .iter()
        .map(|&s| {
            let rgba = match &art {
                Some(a) => image::imageops::resize(a, s, s, image::imageops::FilterType::Lanczos3).into_raw(),
                None => app_icon_rgba(s),
            };
            let mut buf = vec![];
            image::codecs::png::PngEncoder::new(&mut buf).write_image(&rgba, s, s, image::ExtendedColorType::Rgba8)?;
            Ok(buf)
        })
        .collect::<anyhow::Result<Vec<Vec<u8>>>>()?;
    let mut out = vec![0, 0, 1, 0];
    out.extend((SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (&s, data) in SIZES.iter().zip(&pngs) {
        let side = if s >= 256 { 0 } else { s as u8 }; // 0 means 256
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes()); // colour planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for data in pngs {
        out.extend(data);
    }
    std::fs::write(path, out)?;
    Ok(())
}

/// `echoctl dev-icon <dir> [--from logo.png]`: write an .iconset (for `iconutil`), or with a
/// `<dir>.icon` path an Icon Composer document (for `actool`, macOS 26's native icon). With
/// `--from`, the icon is built from that image (the onetest logo); otherwise the drawn mark.
pub fn write_iconset(dir: &std::path::Path, from: Option<&std::path::Path>) -> anyhow::Result<()> {
    if dir.extension().is_some_and(|e| e == "ico") {
        return write_ico(dir, from);
    }
    std::fs::create_dir_all(dir)?;
    #[cfg(target_os = "macos")]
    if let Some(src) = from {
        let logo = image::open(src)?.to_rgba8();
        if dir.extension().is_some_and(|e| e == "icon") {
            // The purple fill comes from icon.json; the octopus is the one layer, and macOS
            // draws the shape, margins and edge itself.
            let (purple, layer) = logo_icon(&logo, 1024, false);
            std::fs::create_dir_all(dir.join("Assets"))?;
            layer.save(dir.join("Assets/octopus.png"))?;
            let c = |i: usize| format!("{:.5}", purple[i] as f32 / 255.0);
            let doc = serde_json::json!({
                "fill": { "solid": format!("srgb:{},{},{},1.00000", c(0), c(1), c(2)) },
                "groups": [{
                    "layers": [{ "image-name": "octopus.png", "name": "octopus", "glass": false }],
                    "shadow": { "kind": "neutral", "opacity": 0.5 },
                    "translucency": { "enabled": false, "value": 0.5 }
                }],
                "supported-platforms": { "squares": ["macOS"] }
            });
            std::fs::write(dir.join("icon.json"), serde_json::to_vec_pretty(&doc)?)?;
            return Ok(());
        }
        let (_, art) = logo_icon(&logo, 1024, true);
        for (name, px) in ICONSET {
            let img = image::imageops::resize(&art, px, px, image::imageops::FilterType::Lanczos3);
            img.save(dir.join(format!("{name}.png")))?;
        }
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    let _ = from;
    for (name, px) in ICONSET {
        std::fs::write(dir.join(format!("{name}.png")), png(&app_icon_rgba(px), px))?;
    }
    Ok(())
}

/// The app icon from the round onetest logo (white octopus on a purple disc), with the octopus
/// as large as fits. `squircle`: the whole icon, the octopus on a purple rounded square in
/// Apple's 824/1024 grid (the .icns for macOS before 26). Otherwise only the octopus, white on
/// transparent, for an Icon Composer layer. Returns the logo's purple too.
#[cfg(any(target_os = "macos", windows))]
fn logo_icon(logo: &image::RgbaImage, px: u32, squircle: bool) -> (image::Rgba<u8>, image::RgbaImage) {
    let (w, h) = logo.dimensions();
    let purple = *logo.get_pixel(w / 2, h / 40); // inside the disc, above the octopus
    // How white a logo pixel is (the octopus), 0..1, from the green channel (purple is low).
    let base = purple[1] as f32;
    let white = |x: u32, y: u32| {
        let p = logo.get_pixel(x, y);
        (((p[1] as f32 - base) / (255.0 - base)).clamp(0.0, 1.0)) * (p[3] as f32 / 255.0)
    };
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if white(x, y) > 0.5 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    // The body is the icon's shape: 824/1024 of the .icns, the whole Icon Composer canvas.
    let body = if squircle { px as f32 * 824.0 / 1024.0 } else { px as f32 };
    let margin = (px as f32 - body) / 2.0;
    let (ow, oh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
    let scale = body * if squircle { 0.84 } else { 0.80 } / ow.max(oh);
    let (ox, oy) = ((px as f32 - ow * scale) / 2.0, (px as f32 - oh * scale) / 2.0);
    let mut out = image::RgbaImage::new(px, px);
    const SS: u32 = 4; // supersampling for smooth edges
    for y in 0..px {
        for x in 0..px {
            let (mut cover, mut wsum) = (0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let fx = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    // Superellipse (n = 5), close to Apple's continuous-corner shape.
                    let u = ((fx - px as f32 / 2.0) / (body / 2.0)).abs();
                    let v = ((fy - px as f32 / 2.0) / (body / 2.0)).abs();
                    if squircle && (u.powi(5) + v.powi(5) > 1.0 || fx < margin || fy < margin) {
                        continue;
                    }
                    cover += 1.0;
                    let lx = x0 as f32 + (fx - ox) / scale;
                    let ly = y0 as f32 + (fy - oy) / scale;
                    if lx >= x0 as f32 && ly >= y0 as f32 && lx < (x1 + 1) as f32 && ly < (y1 + 1) as f32 {
                        wsum += white(lx as u32, ly as u32);
                    }
                }
            }
            let n = (SS * SS) as f32;
            if !squircle {
                if wsum > 0.0 {
                    out.put_pixel(x, y, image::Rgba([255, 255, 255, (wsum / n * 255.0).round() as u8]));
                }
            } else if cover > 0.0 {
                let t = wsum / cover;
                let mix = |c: u8| (c as f32 * (1.0 - t) + 255.0 * t).round() as u8;
                let a = (cover / n * 255.0).round() as u8;
                out.put_pixel(x, y, image::Rgba([mix(purple[0]), mix(purple[1]), mix(purple[2]), a]));
            }
        }
    }
    (purple, out)
}

/// (icon, is_template). Idle is a template icon (follows the macOS menu-bar light/dark look);
/// armed is green and recording is Claude orange so the state reads at a glance.
/// (Windows has no templates: idle is black, or white on a dark taskbar.)
fn icon_for(state: &UiState) -> (Icon, bool) {
    match state {
        UiState::Idle { .. } if taskbar_dark() => (logo(WHITE), true),
        UiState::Idle { .. } => (logo(BLACK), true),
        UiState::Armed => (logo(GREEN), false),
        UiState::Recording { .. } => (logo(ORANGE), false),
    }
}

/// Windows: is the taskbar dark? (The system theme, not the apps' one, colours the taskbar.)
fn taskbar_dark() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::core::w;
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
        let mut light = 1u32;
        let mut len = 4u32;
        let ok = RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut light as *mut u32 as *mut _),
            Some(&mut len),
        )
        .is_ok();
        return ok && light == 0;
    }
    #[allow(unreachable_code)]
    false
}

fn status_text(state: &UiState) -> String {
    match state {
        UiState::Idle { in_call: Some(true) } => "Teams call in progress — not recording".into(),
        UiState::Idle { .. } => "Not recording".into(),
        UiState::Armed => "Armed — recording starts with the next call".into(),
        UiState::Recording { title, since } => format!(
            "● Recording{} · since {}",
            title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default(),
            since.format("%H:%M")
        ),
    }
}

fn open_path(p: &PathBuf) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).status();
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).status();
}

/// Everything the menu shows besides the live status line. The menu is rebuilt only when this
/// changes (checked every few seconds), so it stays cheap.
#[derive(Clone, PartialEq)]
struct Model {
    upcoming: Vec<MeetingContext>,
    /// (menu label, call folder name)
    recent: Vec<(String, String)>,
    unscheduled_manual: bool,
    scheduled_manual: bool,
    keep_audio: bool,
    capture_frames: bool,
    active_model: Option<String>,
    downloaded_models: Vec<&'static str>,
    languages: Vec<String>,
    health: Vec<(String, String, Option<String>)>, // (name, status, settings url)
    home: PathBuf,
    autostart: bool,
    recommended: &'static str,
    /// The model being downloaded, with percent
    downloading: Option<(&'static str, u8)>,
    notes_in: Option<PathBuf>,
    notes_model: String,
    has_claude: bool,
    plugin_installed: bool,
    cli_installed: bool,
    update: Option<String>,
    calendar_account: Option<String>,
    graph_client: (String, bool),
}

const UPCOMING_MAX: usize = 6;
const RECENT_MAX: usize = 5;

fn load_model(health: &[(String, String, Option<String>)]) -> Model {
    let now = Utc::now();
    let today = Local::now().date_naive();
    let mut upcoming: Vec<MeetingContext> = watch::agenda()
        .into_iter()
        .filter(|m| {
            watch::meeting_start(m).is_some_and(|s| {
                s + chrono::Duration::minutes(4) > now && s.with_timezone(&Local).date_naive() == today
            })
        })
        .collect();
    upcoming.sort_by_key(watch::meeting_start);
    upcoming.truncate(UPCOMING_MAX);

    let mut dirs: Vec<PathBuf> = std::fs::read_dir(paths::calls_dir())
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("meta.json").exists()).collect())
        .unwrap_or_default();
    dirs.sort();
    let recording_dir = watch::running_watcher().and_then(|w| w.recording).map(PathBuf::from);
    let recent = dirs
        .iter()
        .rev()
        .take(RECENT_MAX)
        .map(|d| {
            let name = d.file_name().unwrap().to_string_lossy().into_owned();
            let meta: serde_json::Value = std::fs::read(d.join("meta.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            // "2026-10-09_174948-switch-test" → "Oct 09 17:49 · switch test"
            let when = chrono::NaiveDateTime::parse_from_str(name.get(..17).unwrap_or(&name), "%Y-%m-%d_%H%M%S")
                .map(|t| t.format("%b %d %H:%M").to_string())
                .unwrap_or_default();
            let title = meta["title"]
                .as_str()
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .or_else(|| name.get(18..).filter(|t| !t.is_empty()).map(|t| t.replace('-', " ")))
                .unwrap_or("call".into());
            let parse = |k: &str| meta[k].as_str().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
            let minutes = match (parse("started_at"), parse("ended_at")) {
                (Some(a), Some(b)) => format!(" · {} min", ((b - a).num_seconds() + 30).max(60) / 60),
                _ => String::new(),
            };
            let status = if recording_dir.as_deref().is_some_and(|r| d.ends_with(r) || r.ends_with(d)) {
                "● recording"
            } else if d.join(crate::transcribe_marker()).exists() {
                "transcribing…"
            } else if d.join("notes.md").exists() {
                "✓ notes"
            } else if d.join("transcript.md").exists() {
                "✓ transcript"
            } else {
                "not transcribed"
            };
            let label = format!("{when} · {}{minutes} · {status}", ellipsize(&title, 32));
            (label, name)
        })
        .collect();

    let cfg = Config::load().unwrap_or_default();
    let downloaded_models: Vec<&'static str> =
        models::CATALOG.iter().filter(|m| models::model_path(m).exists()).map(|m| m.name).collect();
    let claude = app::claude_status();
    Model {
        upcoming,
        recent,
        unscheduled_manual: !cfg.ask_unscheduled(),
        scheduled_manual: !cfg.ask_scheduled(),
        keep_audio: cfg.keep_audio,
        capture_frames: cfg.capture_frames,
        active_model: cfg.model.clone(),
        downloaded_models,
        languages: cfg.languages.clone(),
        health: health.to_vec(),
        home: paths::home(),
        autostart: app::autostart_enabled(),
        recommended: models::recommend(&models::hardware()).0,
        downloading: app::downloading(),
        notes_in: cfg.notes_in.clone(),
        notes_model: cfg.notes_model_label(),
        has_claude: !claude["claude"].is_null(),
        plugin_installed: claude["plugin_installed"] == true,
        cli_installed: app::cli_installed(),
        calendar_account: crate::graph::account(),
        graph_client: {
            let (id, _) = crate::graph::app_registration();
            let default = id == crate::graph::DEFAULT_CLIENT_ID;
            (id, default)
        },
        update: crate::update::AVAILABLE.lock().unwrap().as_ref().map(|r| r.version.clone()),
    }
}

/// Permission checks for the Health submenu (no probe: that needs audio playing).
fn health() -> Vec<(String, String, Option<String>)> {
    match permissions::report() {
        Ok(r) => r
            .checks
            .iter()
            .map(|c| {
                let status = match c.status {
                    permissions::Status::Granted => "✓",
                    permissions::Status::Denied => "✗",
                    _ => "?",
                };
                (c.name.to_string(), status.to_string(), c.settings_url.map(str::to_owned))
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Display names for language codes offered in the Record menu.
fn language_name(code: &str) -> String {
    match code {
        "en" => "English".into(),
        "uk" => "Українська".into(),
        "ru" => "Русский".into(),
        "es" => "Español".into(),
        "de" => "Deutsch".into(),
        "fr" => "Français".into(),
        "pl" => "Polski".into(),
        other => other.to_string(),
    }
}

#[derive(Clone)]
enum Action {
    Record,
    CheckUpdates,
    RecordIn(String),
    Stop,
    ArmMeeting(Box<MeetingContext>),
    JoinMeeting(Box<MeetingContext>),
    Open(PathBuf),
    SetUnscheduled(&'static str),
    SetScheduled(&'static str),
    ToggleKeepAudio,
    ToggleFrames,
    UseModel(&'static str),
    RemoveModel(&'static str),
    NotesAnywhere,
    NotesChoose,
    ConnectClaude,
    InstallCli,
    Update,
    EditProfile,
    AddNote,
    /// Open the Echo window (at a call, by folder name).
    OpenWindow(Option<String>),
    CalendarLogin,
    CalendarLogout,
    CalendarSync,
    CalendarClientId,
    NotesModel,
    Recheck,
    AskPermissions,
    OpenUrl(String),
    ChangeHome,
    ToggleAutostart,
    DownloadModel(&'static str),
    Quit,
}

struct Ui {
    stop_flag: Arc<AtomicBool>,
    quit: MenuItem,
    tray: TrayIcon,
    status: MenuItem,
    record: Submenu,
    stop: MenuItem,
    note: MenuItem,
    actions: HashMap<MenuId, Action>,
    state: Option<UiState>,
    /// The taskbar theme the icon was drawn for (Windows: the idle icon follows it).
    dark: bool,
    model: Option<Model>,
    health: Vec<(String, String, Option<String>)>,
    last_refresh: Instant,
    /// Set by a click: open (or focus) the window, at this call; the event loop does it.
    open_window: Option<Option<String>>,
}

fn local_hm(m: &MeetingContext) -> String {
    watch::meeting_start(m).map(|s| s.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_default()
}

fn ellipsize(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

impl Ui {
    fn new(stop: Arc<AtomicBool>) -> tray_icon::Result<Self> {
        let (icon, _) = icon_for(&UiState::Idle { in_call: None });
        let builder = TrayIconBuilder::new().with_tooltip("Echo");
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        let tray = builder.build()?;
        let mut ui = Ui {
            stop_flag: stop,
            quit: MenuItem::new("", true, None),
            tray,
            status: MenuItem::new("", false, None),
            record: Submenu::new("", true),
            stop: MenuItem::new("", false, None),
            note: MenuItem::new("", false, None),
            actions: HashMap::new(),
            state: None,
            dark: taskbar_dark(),
            model: None,
            health: health(),
            last_refresh: Instant::now(),
            open_window: None,
        };
        ui.rebuild(load_model(&ui.health));
        Ok(ui)
    }

    fn item(&mut self, text: &str, enabled: bool, action: Option<Action>) -> MenuItem {
        let it = MenuItem::new(text, enabled, None);
        if let Some(a) = action {
            self.actions.insert(it.id().clone(), a);
        }
        it
    }

    fn check(&mut self, text: &str, checked: bool, action: Action) -> CheckMenuItem {
        let it = CheckMenuItem::new(text, true, checked, None);
        self.actions.insert(it.id().clone(), action);
        it
    }

    /// Rebuild the whole menu from `model` (the live status/record/stop items are recreated and
    /// then refreshed from the current state).
    fn rebuild(&mut self, model: Model) {
        self.actions.clear();
        let menu = Menu::new();
        self.status = MenuItem::new("", false, None);
        // Record ▸ Auto (workspace languages) / one language pinned for this call
        self.record = Submenu::new("Record this call", true);
        let langs = if model.languages.is_empty() { "any language".to_string() } else { model.languages.join(", ") };
        let auto = self.item(&format!("Auto ({langs})"), true, Some(Action::Record));
        let _ = self.record.append_items(&[&auto, &PredefinedMenuItem::separator()]);
        for code in &model.languages {
            let it = self.item(&format!("Only {}", language_name(code)), true, Some(Action::RecordIn(code.clone())));
            let _ = self.record.append(&it);
        }
        self.stop = self.item("Stop recording", false, Some(Action::Stop));
        self.note = self.item("Add a note…", false, Some(Action::AddNote));
        let sep = PredefinedMenuItem::separator;
        let open = self.item("Open Echo…", true, Some(Action::OpenWindow(None)));
        let _ = menu.append_items(&[&self.status, &sep(), &self.record, &self.stop, &self.note, &sep(), &open, &sep()]);

        // Upcoming meetings (today)
        let _ = menu.append(&MenuItem::new("Upcoming", false, None));
        if model.upcoming.is_empty() {
            let _ = menu.append(&MenuItem::new("   No more meetings today", false, None));
        }
        for m in &model.upcoming {
            let title = format!("{}  {}", local_hm(m), ellipsize(m.subject.as_deref().unwrap_or("Meeting"), 40));
            let sub = Submenu::new(title, true);
            let last = m.subject.as_deref().map(crate::calls::recap).unwrap_or_default();
            if !last.is_empty() {
                let _ = sub.append(&MenuItem::new("Last time:", false, None));
                for b in &last {
                    let _ = sub.append(&MenuItem::new(format!("   • {b}"), false, None));
                }
                let _ = sub.append(&sep());
            }
            let arm = self.item("Record when it starts", true, Some(Action::ArmMeeting(Box::new(m.clone()))));
            let _ = sub.append(&arm);
            if m.join_url.is_some() {
                let join = self.item("Join and record", true, Some(Action::JoinMeeting(Box::new(m.clone()))));
                let _ = sub.append(&join);
            }
            let _ = menu.append(&sub);
        }
        let _ = menu.append(&sep());

        // Recent calls
        let _ = menu.append(&MenuItem::new("Recent calls", false, None));
        if model.recent.is_empty() {
            let _ = menu.append(&MenuItem::new("   No recordings yet", false, None));
        }
        for (label, name) in &model.recent {
            let it = self.item(&format!("   {label}"), true, Some(Action::OpenWindow(Some(name.clone()))));
            let _ = menu.append(&it);
        }
        let folder = self.item("Open calls folder", true, Some(Action::Open(paths::calls_dir())));
        let _ = menu.append_items(&[&folder, &sep()]);

        // Settings
        let settings = Submenu::new("Settings", true);
        let _ = settings.append(&MenuItem::new("Calendar meetings", false, None));
        let s_ask = self.check("   Ask before they start", !model.scheduled_manual, Action::SetScheduled("ask"));
        let s_manual = self.check("   Only when I click Record", model.scheduled_manual, Action::SetScheduled("manual"));
        let _ = settings.append_items(&[&s_ask, &s_manual, &sep()]);
        let _ = settings.append(&MenuItem::new("Calls not on the calendar", false, None));
        let ask = self.check("   Ask when they start", !model.unscheduled_manual, Action::SetUnscheduled("ask"));
        let manual = self.check("   Only when I click Record", model.unscheduled_manual, Action::SetUnscheduled("manual"));
        let _ = settings.append_items(&[&ask, &manual, &sep()]);
        let keep = self.check("Keep audio after transcription", model.keep_audio, Action::ToggleKeepAudio);
        let frames = self.check("Save snapshots of shared screens", model.capture_frames, Action::ToggleFrames);
        let _ = settings.append(&frames);
        let _ = settings.append_items(&[&keep, &sep()]);
        let langs = if model.languages.is_empty() { "any".to_string() } else { model.languages.join(", ") };
        let _ = settings.append_items(&[&sep(), &MenuItem::new(format!("Languages: {langs}"), false, None)]);
        let profile = self.item("Edit my profile (for notes)…", true, Some(Action::EditProfile));
        let _ = settings.append(&profile);
        let cfg = self.item("Open settings file", true, Some(Action::Open(paths::state_dir().join("config.json"))));
        let _ = settings.append_items(&[&cfg, &sep()]);
        let _ = settings.append(&MenuItem::new(format!("Data folder: {}", ellipsize(&model.home.display().to_string(), 44)), false, None));
        let open_home = self.item("   Open data folder", true, Some(Action::Open(model.home.clone())));
        let recording = matches!(self.state, Some(UiState::Recording { .. }));
        let change = self.item("   Change data folder…", !recording, Some(Action::ChangeHome));
        let login = self.check("Start Echo at login", model.autostart, Action::ToggleAutostart);
        let _ = settings.append_items(&[&open_home, &change, &sep(), &login]);
        if !model.cli_installed {
            let cli = self.item("Install command-line tool (Echo)…", true, Some(Action::InstallCli));
            let _ = settings.append(&cli);
        }

        // Speech models: every catalog entry with Use / Download / Remove.
        let models_menu = Submenu::new("Speech models", true);
        for m in models::CATALOG {
            let downloaded = model.downloaded_models.contains(&m.name);
            let active = model.active_model.as_deref() == Some(m.name);
            let mark = if active { "✓ " } else if downloaded { "   " } else { "↓ " };
            let sub = Submenu::new(format!("{mark}{} · {} MB", m.name, m.size_mb), true);
            let _ = sub.append(&MenuItem::new(m.summary, false, None));
            if m.name == model.recommended {
                let _ = sub.append(&MenuItem::new("Recommended for this computer", false, None));
            }
            if let (Some(role), false) = (models::helper_role(m.name), active) {
                let _ = sub.append(&MenuItem::new(format!("Also used: {role}"), false, None));
            }
            let _ = sub.append(&sep());
            if active {
                let _ = sub.append(&MenuItem::new("In use", false, None));
            } else if downloaded {
                let use_it = self.item("Use this model", true, Some(Action::UseModel(m.name)));
                let remove = self.item(&format!("Remove ({} MB)…", m.size_mb), true, Some(Action::RemoveModel(m.name)));
                let _ = sub.append_items(&[&use_it, &remove]);
            } else {
                let it = match model.downloading {
                    Some((name, pct)) if name == m.name => MenuItem::new(format!("Downloading… {pct}%"), false, None),
                    Some(_) => MenuItem::new("Download (another download is running)", false, None),
                    None => self.item(&format!("Download ({} MB)", m.size_mb), true, Some(Action::DownloadModel(m.name))),
                };
                let _ = sub.append(&it);
            }
            let _ = models_menu.append(&sub);
        }
        if let Some((name, pct)) = model.downloading {
            let _ = models_menu.append_items(&[&sep(), &MenuItem::new(format!("Downloading {name}: {pct}%"), false, None)]);
        }

        // Claude Code: the plugin (notes, live pane) and where it writes notes.
        let claude = Submenu::new(if model.plugin_installed { "Claude Code ✓" } else { "Claude Code" }, true);
        if !model.has_claude {
            let _ = claude.append(&MenuItem::new("Claude Code is not installed", false, None));
        } else if model.plugin_installed {
            let _ = claude.append(&MenuItem::new("Echo plugin installed", false, None));
        } else {
            let connect = self.item("Connect Claude Code (install the plugin)", true, Some(Action::ConnectClaude));
            let _ = claude.append(&connect);
        }
        let _ = claude.append_items(&[&sep(), &MenuItem::new("Write call notes in", false, None)]);
        let any = self.check("   Any Claude Code session", model.notes_in.is_none(), Action::NotesAnywhere);
        let _ = claude.append(&any);
        if let Some(dir) = &model.notes_in {
            let only = self.check(&format!("   Only sessions in {}", ellipsize(&dir.display().to_string(), 40)), true, Action::NotesChoose);
            let _ = claude.append(&only);
        }
        let choose = self.item("   Choose a folder…", true, Some(Action::NotesChoose));
        let _ = claude.append(&choose);
        let notes_model = self.item(&format!("Notes model: {}…", model.notes_model), model.has_claude, Some(Action::NotesModel));
        let _ = claude.append_items(&[&sep(), &notes_model]);

        // Health
        let all_ok = model.health.iter().all(|(_, s, _)| s == "✓");
        let health = Submenu::new(if all_ok { "Health ✓" } else { "Health ⚠" }, true);
        for (name, status, url) in &model.health {
            let action = url.clone().filter(|_| status != "✓").map(Action::OpenUrl);
            let it = self.item(&format!("{status}  {name}"), action.is_some(), action);
            let _ = health.append(&it);
        }
        let askp = self.item("Ask for permissions…", !all_ok, Some(Action::AskPermissions));
        let recheck = self.item("Re-check permissions", true, Some(Action::Recheck));
        let _ = health.append_items(&[&sep(), &askp, &recheck]);
        // Calendar: the app's own Microsoft 365 sign-in (read-only).
        let calendar = Submenu::new(if model.calendar_account.is_some() { "Calendar ✓" } else { "Calendar" }, true);
        match &model.calendar_account {
            Some(who) => {
                let _ = calendar.append(&MenuItem::new(format!("Microsoft 365: {who}"), false, None));
                let sync = self.item("Refresh now", true, Some(Action::CalendarSync));
                let out = self.item("Sign out", true, Some(Action::CalendarLogout));
                let _ = calendar.append_items(&[&sync, &out]);
            }
            None => {
                let login = self.item("Connect Microsoft 365…", true, Some(Action::CalendarLogin));
                let _ = calendar.append(&login);
            }
        }
        let (id, is_default) = &model.graph_client;
        let label = if *is_default { "App: CodeMie (default)".to_string() } else { format!("App: {}…", id.chars().take(8).collect::<String>()) };
        let _ = calendar.append_items(&[&sep(), &MenuItem::new(label, false, None)]);
        let change = self.item("Change app (client) ID…", true, Some(Action::CalendarClientId));
        let _ = calendar.append(&change);
        let _ = menu.append_items(&[&settings, &models_menu, &calendar, &claude, &health, &sep()]);
        let up = match &model.update {
            Some(v) => self.item(&format!("Update to {v}…"), true, Some(Action::Update)),
            None => self.item("Check for updates…", true, Some(Action::CheckUpdates)),
        };
        let _ = menu.append_items(&[&up, &sep()]);
        // Quit: stops the watcher (a recording in progress is finished and saved first).
        self.quit = self.item("Quit Echo", true, Some(Action::Quit));
        let _ = menu.append_items(&[&self.quit, &MenuItem::new(format!("Echo {}", env!("CARGO_PKG_VERSION")), false, None)]);

        self.tray.set_menu(Some(Box::new(menu)));
        self.model = Some(model);
        if let Some(s) = self.state.take() {
            self.apply(s); // re-render status/record/stop on the new items
        }
    }

    fn refresh_model(&mut self, force: bool) {
        if !force && self.last_refresh.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.last_refresh = Instant::now();
        let m = load_model(&self.health);
        if self.model.as_ref() != Some(&m) {
            self.rebuild(m);
        }
    }

    fn apply(&mut self, state: UiState) {
        let dark = taskbar_dark();
        if self.state.as_ref() == Some(&state) && self.dark == dark {
            return;
        }
        self.dark = dark;
        let (icon, template) = icon_for(&state);
        #[cfg(target_os = "macos")]
        let _ = if template { self.tray.set_icon_templated(Some(icon)) } else { self.tray.set_icon(Some(icon)) };
        #[cfg(not(target_os = "macos"))]
        let _ = (template, self.tray.set_icon(Some(icon)));
        let text = status_text(&state);
        self.status.set_text(&text);
        let _ = self.tray.set_tooltip(Some(format!("Echo — {text}")));
        match &state {
            UiState::Idle { in_call } => {
                self.record.set_text(if *in_call == Some(true) { "Record this call" } else { "Record next call" });
                self.record.set_enabled(true);
                self.stop.set_text("Stop recording");
                self.stop.set_enabled(false);
            }
            UiState::Armed => {
                self.record.set_enabled(false);
                self.stop.set_text("Cancel");
                self.stop.set_enabled(true);
            }
            UiState::Recording { .. } => {
                self.record.set_enabled(false);
                self.stop.set_text("Stop recording");
                self.stop.set_enabled(true);
            }
        }
        self.note.set_enabled(matches!(state, UiState::Recording { .. }));
        if self.quit.is_enabled() {
            self.quit.set_text(if matches!(state, UiState::Recording { .. }) {
                "Quit Echo (saves the current recording)"
            } else {
                "Quit Echo"
            });
        }
        self.state = Some(state);
    }

    /// "Update to …" (menu, or a click on the "Update available" notification): ask, install.
    fn start_update(&self) {
        let Some(r) = crate::update::AVAILABLE.lock().unwrap().clone() else { return };
        let stop = self.stop_flag.clone();
        std::thread::spawn(move || {
            let msg = format!("Install Echo {}?\n\nEcho restarts; nothing is recorded meanwhile.", r.version);
            if app::ask(&msg, &["Later", "Update"], "Update").as_deref() != Some("Update") {
                return;
            }
            match crate::update::install(&r, std::process::id()) {
                Ok(()) => stop.store(true, std::sync::atomic::Ordering::SeqCst), // the swap runs after exit
                Err(e) => app::notify("Update failed", &format!("{e:#}")),
            }
        });
    }

    fn click(&mut self, id: &MenuId) {
        let Some(action) = self.actions.get(id).cloned() else { return };
        match action {
            Action::Record => drop(watch::arm(None, None, None)),
            Action::RecordIn(code) => drop(watch::arm(None, None, Some(vec![code]))),
            Action::Stop => drop(watch::stop()),
            Action::ArmMeeting(m) => drop(watch::arm(m.subject.clone(), Some(*m), None)),
            Action::JoinMeeting(m) => drop(watch::join(*m)),
            Action::Open(p) => open_path(&p),
            Action::SetUnscheduled(v) => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.unscheduled = Some(v.to_string());
                let _ = cfg.save();
            }
            Action::SetScheduled(v) => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.scheduled = Some(v.to_string());
                let _ = cfg.save();
            }
            Action::ToggleFrames => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.capture_frames = !cfg.capture_frames;
                let _ = cfg.save();
                #[cfg(target_os = "macos")]
                if cfg.capture_frames && !crate::frames::permitted() {
                    // macOS shows its prompt once; afterwards it only lists the app in Settings.
                    std::thread::spawn(|| {
                        if !crate::frames::request() {
                            app::notify("Screen Recording needed", "Enable Echo in Privacy & Security → Screen & System Audio Recording, then restart Echo.");
                        }
                    });
                }
            }
            Action::ToggleKeepAudio => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.keep_audio = !cfg.keep_audio;
                let _ = cfg.save();
            }
            Action::UseModel(name) => drop(models::use_model(name)),
            Action::RemoveModel(name) => {
                std::thread::spawn(move || {
                    let size = models::find(name).map(|m| m.size_mb).unwrap_or(0);
                    let warning = models::helper_role(name).map(|r| format!("\n\nNote: {r} — without it they fall back to the slower active model.")).unwrap_or_default();
                    let msg = format!("Remove the speech model {name}?\n\nFrees {size} MB. You can download it again later.{warning}");
                    if app::ask(&msg, &["Cancel", "Remove"], "Remove").as_deref() == Some("Remove") {
                        if let Err(e) = models::remove(name, true) {
                            app::notify("Could not remove the model", &format!("{e:#}"));
                        }
                    }
                });
            }
            Action::NotesAnywhere => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.notes_in = None;
                let _ = cfg.save();
            }
            Action::NotesChoose => {
                std::thread::spawn(|| {
                    if let Some(dir) = app::choose_folder("Claude Code sessions opened in this folder write the call notes") {
                        let Ok(mut cfg) = Config::load() else { return };
                        cfg.notes_in = Some(dunce::canonicalize(&dir).unwrap_or(dir));
                        let _ = cfg.save();
                    }
                });
            }
            Action::ConnectClaude => {
                std::thread::spawn(|| match app::connect_claude("user", None) {
                    Ok(()) => app::notify("Claude Code connected", "New Claude Code sessions now write your call notes."),
                    Err(e) => app::notify("Could not connect Claude Code", &format!("{e:#}")),
                });
            }
            Action::Update => {
                self.start_update();
                return;
            }
            Action::CheckUpdates => {
                // Found: the same prompt as "Update to …" (the tray loop starts it).
                std::thread::spawn(|| match crate::update::check() {
                    Ok(Some(_)) => app::request_update(),
                    Ok(None) => app::notify("Echo is up to date", &format!("Echo {} is the latest version.", env!("CARGO_PKG_VERSION"))),
                    Err(e) => app::notify("Couldn't check for updates", &format!("{e:#}")),
                });
                return;
            }
            Action::CalendarLogin => {
                std::thread::spawn(|| match crate::graph::login() {
                    Ok(who) => {
                        let _ = crate::calendar::sync(true);
                        app::notify("Calendar connected", &who);
                    }
                    Err(e) => app::notify("Calendar sign-in failed", &format!("{e:#}")),
                });
                return;
            }
            Action::CalendarLogout => drop(crate::graph::logout()),
            Action::CalendarSync => {
                std::thread::spawn(|| drop(crate::calendar::sync(true)));
            }
            Action::CalendarClientId => {
                std::thread::spawn(|| {
                    let (current, _) = crate::graph::app_registration();
                    let msg = "Microsoft 365 app (client) ID for the calendar sign-in.\n\nYour IT can register a public client with redirect http://localhost and the delegated Calendars.Read permission. Type \"default\" for CodeMie.";
                    if let Some(id) = app::ask_text(msg, &current, "Save") {
                        let Ok(mut cfg) = Config::load() else { return };
                        let id = id.trim().to_string();
                        cfg.graph_client_id = (!id.eq_ignore_ascii_case("default") && id != crate::graph::DEFAULT_CLIENT_ID).then_some(id);
                        let _ = cfg.save();
                    }
                });
                return;
            }
            Action::NotesModel => {
                std::thread::spawn(|| {
                    let current = Config::load().unwrap_or_default().notes_model_label();
                    let msg = "Model Claude Code uses when Echo writes call notes:\n\nsonnet, opus or haiku, a full model name, or \"default\" to use Claude Code's own setting (local models, a gateway, Bedrock or Vertex).";
                    if let Some(m) = app::ask_text(msg, &current, "Save") {
                        let Ok(mut cfg) = Config::load() else { return };
                        let m = m.trim().to_string();
                        cfg.notes_model = (!m.is_empty() && m != crate::config::DEFAULT_NOTES_MODEL).then_some(m);
                        let _ = cfg.save();
                    }
                });
                return;
            }
            Action::OpenWindow(call) => {
                self.open_window = Some(call);
                return;
            }
            Action::AddNote => {
                // Your own note: it becomes the backbone of the call's notes ("# …" = heading).
                std::thread::spawn(|| {
                    if let Some(t) = app::ask_text("Note for this call (start with # for a heading):", "", "Add") {
                        if let Err(e) = crate::calls::note(&t) {
                            app::notify("Note not saved", &format!("{e:#}"));
                        }
                    }
                });
                return;
            }
            Action::EditProfile => {
                if let Ok(p) = crate::calls::ensure_me_profile() {
                    open_path(&p);
                }
            }
            Action::InstallCli => {
                std::thread::spawn(|| match app::install_cli() {
                    Ok(()) => app::notify("Command-line tool installed", "Type `echoctl` in a new terminal."),
                    Err(e) => app::notify("Command-line tool not installed", &format!("{e:#}")),
                });
            }
            Action::Recheck => self.health = health(),
            Action::AskPermissions => app::request_permissions(),
            Action::DownloadModel(name) => app::download_model_in_background(name),
            Action::ToggleAutostart => drop(app::set_autostart(!app::autostart_enabled())),
            Action::ChangeHome => {
                // The picker blocks: run it off the event loop, then restart on the new folder.
                let stop = self.stop_flag.clone();
                std::thread::spawn(move || {
                    let Some(dir) = app::choose_folder("Folder for Echo calls and notes") else { return };
                    if dir == paths::home() || paths::set_home(&dir).is_err() {
                        return;
                    }
                    app::RESTART.store(true, std::sync::atomic::Ordering::SeqCst);
                    stop.store(true, std::sync::atomic::Ordering::SeqCst);
                });
                return;
            }
            Action::OpenUrl(u) => drop(permissions::open_settings(&u)),
            Action::Quit => {
                self.quit.set_text("Quitting…");
                self.quit.set_enabled(false);
                self.stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                return;
            }
        }
        self.refresh_model(true);
    }
}

/// macOS: a menu-bar app has no Dock icon; while its window is open it gets one (and a place
/// in Cmd-Tab), like other menu-bar apps with a main window.
fn set_dock_icon(target: &tao::event_loop::EventLoopWindowTarget<UiEvent>, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopWindowTargetExtMacOS};
        target.set_activation_policy_at_runtime(if visible { ActivationPolicy::Regular } else { ActivationPolicy::Accessory });
        if visible {
            set_app_icon();
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (target, visible);
}

/// The onetest logo as the Dock icon of a dev build, which has no bundle icon. Echo.app keeps
/// its icns (the full rounded-square icon).
#[cfg(target_os = "macos")]
fn set_app_icon() {
    if crate::app::bundle_path().is_some() {
        return;
    }
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};
    const LOGO: &[u8] = include_bytes!("../assets/onetest-logo-512.png");
    let Some(mtm) = MainThreadMarker::new() else { return }; // the tao loop runs on the main thread
    let data = NSData::with_bytes(LOGO);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: called on the main thread (checked above) with a valid NSImage.
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

/// Run the watcher with a tray icon. Never returns; exits once the watcher loop has finished
/// (and closed any recording).
pub fn run(app: TargetApp, stop: Arc<AtomicBool>) -> ! {
    let stop_for_ui = stop.clone();
    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UiEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory); // no Dock icon
        crate::app::notifications::init();
    }

    let proxy = event_loop.create_proxy();
    let p = proxy.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = p.send_event(UiEvent::Menu(e.id));
    }));
    let p = proxy.clone();
    std::thread::spawn(move || {
        if let Err(e) = watch::run_loop(app, stop, |s| {
            let _ = p.send_event(UiEvent::State(s));
        }) {
            eprintln!("watcher failed: {e:#}");
        }
        let _ = p.send_event(UiEvent::Quit);
    });

    let mut ui: Option<Ui> = None;
    let mut win: Option<crate::window::EchoWindow> = None;
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. }
                if win.as_ref().is_some_and(|w| w.window.id() == window_id) =>
            {
                win = None; // closing the window leaves Echo running in the menu bar
                set_dock_icon(target, false);
            }
            Event::UserEvent(UiEvent::Ipc(msg)) if crate::window::handle(&msg) => {
                // The action changed something: refresh the page and the menu.
                if let Some(w) = &win {
                    w.refresh();
                }
                if let Some(u) = ui.as_mut() {
                    u.refresh_model(true);
                }
            }
            // macOS: create the icon only once the event loop is running.
            Event::NewEvents(StartCause::Init) => match Ui::new(stop_for_ui.clone()) {
                Ok(u) => ui = Some(u),
                Err(e) => eprintln!("tray icon unavailable: {e}"),
            },
            Event::UserEvent(UiEvent::State(s)) => {
                if let Some(u) = ui.as_mut() {
                    u.apply(s);
                    u.refresh_model(false);
                    if app::take_update_request() {
                        u.start_update();
                    }
                    if let Some(call) = app::take_open_request() {
                        u.open_window = Some(call);
                        let _ = proxy.send_event(UiEvent::Menu(MenuId::new("echo-open-window")));
                    }
                }
            }
            Event::UserEvent(UiEvent::Menu(id)) => {
                if let Some(u) = ui.as_mut() {
                    u.click(&id);
                    if let Some(call) = u.open_window.take() {
                        match &win {
                            Some(w) => {
                                if let Some(c) = &call {
                                    w.show_call(c);
                                }
                                w.window.set_visible(true);
                                w.window.set_focus();
                            }
                            None => {
                                let p = proxy.clone();
                                match crate::window::open(target, call.as_deref(), move |m| drop(p.send_event(UiEvent::Ipc(m)))) {
                                    Ok(w) => {
                                        set_dock_icon(target, true);
                                        w.window.set_focus();
                                        win = Some(w);
                                    }
                                    Err(e) => app::notify("Could not open the Echo window", &e.to_string()),
                                }
                            }
                        }
                    }
                }
            }
            Event::UserEvent(UiEvent::Quit) => {
                if app::RESTART.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = app::spawn_app(true);
                }
                *control_flow = ControlFlow::Exit
            }
            _ => {}
        }
    })
}
