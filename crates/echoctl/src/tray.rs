//! Menu-bar (macOS) / notification-area (Windows) icon of the echoctl app.
//!
//! The app's whole UI: status, Record / Stop, upcoming meetings, recent calls, settings,
//! permissions and Quit. Clicks call the same `arm` / `stop` as the CLI and Claude Code's
//! skills, so every client sees the same events.
//!
//! macOS needs the event loop on the main thread, so the watcher loop runs on a worker thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// Close the window, quit, update or change the data folder — once the notes typed in the
    /// window are saved (`Leaving`).
    Leave(Then),
    /// The window's page didn't answer that save handshake (its number) in time.
    LeaveTimeout(u64),
    /// After a handshake that failed, the user chose: go on anyway (true) or keep Echo (false) —
    /// or couldn't be asked (why).
    LeaveDecided(Result<bool, String>),
    Quit,
}

/// What a leave does once the window has saved the notes it holds.
#[derive(Debug)]
enum Then {
    CloseWindow,
    Quit,
    /// `echoctl app quit` asked (it waits for the outcome).
    QuitAsked,
    Update(crate::update::Staged),
    ChangeHome(PathBuf),
}

impl Then {
    fn question(&self) -> &'static str {
        match self {
            Then::CloseWindow => "Close the window anyway?",
            Then::Quit | Then::QuitAsked => "Quit Echo anyway?",
            Then::Update(_) => "Install the update anyway?",
            Then::ChangeHome(_) => "Change the data folder anyway?",
        }
    }
}

/// Where the leave stands: one at a time, from the window's save handshake to Echo stopping. A
/// leave asked meanwhile takes the pending one's place (`takes_over`) or is dropped — while the
/// user is asked, it waits for the answer instead (`queued`), so a question is never answered for
/// another leave; once Echo is stopping, every one is dropped. The event loop does what each step
/// says (`act`).
#[derive(Default)]
struct Leaving {
    phase: Phase,
    handshakes: u64,
    /// Asked while the user is asked about another leave: started after that answer (it saves and
    /// asks its own question).
    queued: Option<Then>,
    /// `quit.asking` is written (`asking_quit`).
    asking_file: bool,
}

#[derive(Debug, Default)]
enum Phase {
    #[default]
    Idle,
    /// The window's page saves the notes it holds (`prepareClose`, handshake `n`; 5 s at most).
    Saving(Then, u64),
    /// The save failed or the page didn't answer: the user is asked whether to go on anyway.
    Asking(Then),
    /// A quit, an update or a folder change went ahead: Echo is stopping.
    Going,
}

/// What the event loop does for the leave (`act`).
#[derive(Debug)]
enum Step {
    Nothing,
    /// The page saves its notes (`prepareClose(n, close)`; a close explains a failure itself).
    Save { n: u64, close: bool },
    /// Do it (`go`); then `went` says whether Echo stops.
    Go(Then),
    /// Ask the user whether to go on anyway (`ask_to_leave`).
    Ask(String),
    /// It didn't happen: the editors writable again, and whoever waits on a quit told why.
    Stay(Then, String),
    /// Close the window only: the leave still waits for the user's answer.
    CloseWindow,
}

impl Leaving {
    /// A leave is asked for; `window`: a page holds notes to save first.
    fn start(&mut self, then: Then, window: bool) -> Step {
        match &mut self.phase {
            Phase::Going => Step::Nothing,
            Phase::Saving(pending, _) => {
                if takes_over(pending, &then) {
                    *pending = then;
                }
                Step::Nothing
            }
            Phase::Asking(pending) => {
                if takes_over(pending, &then) && self.queued.as_ref().is_none_or(|q| takes_over(q, &then)) {
                    self.queued = Some(then);
                }
                Step::Nothing
            }
            Phase::Idle if !window => Step::Go(then),
            Phase::Idle => {
                self.handshakes += 1;
                let close = matches!(then, Then::CloseWindow);
                self.phase = Phase::Saving(then, self.handshakes);
                Step::Save { n: self.handshakes, close }
            }
        }
    }

    /// The page answered handshake `n` (`ok`: its notes are saved), or didn't in time (None). An
    /// answer or a timeout for another handshake changes nothing.
    fn answered(&mut self, n: u64, ok: Option<bool>) -> Step {
        if !matches!(self.phase, Phase::Saving(_, m) if m == n) {
            return Step::Nothing;
        }
        let Phase::Saving(then, _) = std::mem::take(&mut self.phase) else { return Step::Nothing };
        match after_handshake(then, ok) {
            Next::Go(t) => Step::Go(t),
            Next::Stay => Step::Nothing,
            Next::Ask(t, why) => {
                let question = format!("{why}\n\n{}", t.question());
                self.phase = Phase::Asking(t);
                Step::Ask(question)
            }
        }
    }

    /// The user's answer to `Ask` (Err: they couldn't be asked).
    fn decided(&mut self, answer: Result<bool, String>) -> Step {
        match (std::mem::take(&mut self.phase), answer) {
            (Phase::Asking(then), Ok(true)) => Step::Go(then),
            (Phase::Asking(then), Ok(false)) => Step::Stay(then, "notes typed in its window couldn't be saved, and Echo was kept open".into()),
            (Phase::Asking(then), Err(why)) => {
                Step::Stay(then, format!("notes typed in its window couldn't be saved, and Echo couldn't ask whether to go on ({why})"))
            }
            (other, _) => {
                self.phase = other;
                Step::Nothing
            }
        }
    }

    /// The page's Close without saving (after a close it couldn't save): a leave still saving goes
    /// on without the notes; one waiting for the user's answer keeps waiting, the window closes.
    fn close_now(&mut self) -> Step {
        match std::mem::take(&mut self.phase) {
            Phase::Idle => Step::Go(Then::CloseWindow),
            Phase::Saving(then, _) => Step::Go(then),
            asking @ Phase::Asking(_) => {
                self.phase = asking;
                Step::CloseWindow
            }
            Phase::Going => {
                self.phase = Phase::Going;
                Step::Nothing
            }
        }
    }

    /// What `go` did: Echo stops, or it stays (a window closed; an update or a folder change that
    /// failed).
    fn went(&mut self, stopping: bool) {
        self.phase = if stopping { Phase::Going } else { Phase::Idle };
    }

    /// A leave that waited for the user's answer about another, once that one is done.
    fn next(&mut self) -> Option<Then> {
        if matches!(self.phase, Phase::Asking(_)) { None } else { self.queued.take() }
    }

    /// The quit `echoctl app quit` waits for is held up by a question to the user (`quit.asking`
    /// while so: the CLI keeps waiting).
    /// Echo is stopping, or the user is being asked whether it may (a quit, an update, a folder change):
    /// no window opens now — one opened then would lose what is typed in it.
    fn may_open(&self) -> bool {
        !matches!(self.phase, Phase::Going | Phase::Asking(Then::Quit | Then::QuitAsked | Then::Update(_) | Then::ChangeHome(_)))
    }

    fn asking_quit(&self) -> bool {
        matches!(self.phase, Phase::Asking(Then::QuitAsked)) || matches!(self.queued, Some(Then::QuitAsked))
    }
}

/// A leave asked while another is pending (saving, or asking the user): does it take that one's
/// place? Anything replaces a close; a quit replaces an update or a folder change (what was asked
/// last wins); `echoctl app quit` replaces a quit (it waits for the outcome). Anything else is
/// dropped: what is pending stops Echo too. So macOS's own quit, which waits for an answer, always
/// rides on a quit that ends in `go` (answered yes on exit) or `Stay` (answered no).
fn takes_over(pending: &Then, new: &Then) -> bool {
    matches!(
        (pending, new),
        (Then::CloseWindow, _) | (Then::Update(_) | Then::ChangeHome(_), Then::Quit | Then::QuitAsked) | (Then::Quit, Then::QuitAsked)
    )
}

/// What follows the window's answer to the save handshake (or its silence).
#[derive(Debug)]
enum Next {
    Go(Then),
    Ask(Then, &'static str),
    /// A close the page explains itself (Try again / Close without saving).
    Stay,
}

/// `close_ready`: saved → go on; not saved → the page says so for a close, the user is asked
/// for anything else. No answer (`ok` None) → the user is asked, a close too.
fn after_handshake(then: Then, ok: Option<bool>) -> Next {
    match (ok, then) {
        (Some(true), t) => Next::Go(t),
        (Some(false), Then::CloseWindow) => Next::Stay,
        (Some(false), t) => Next::Ask(t, "Notes typed in the Echo window couldn't be saved."),
        (None, t) => Next::Ask(t, "The Echo window didn't confirm that the notes typed in it are saved."),
    }
}

/// Ask whether to leave anyway, off the event loop (`LeaveDecided`: once, also if the dialog's
/// thread fails — the leave would wait for ever).
fn ask_to_leave(question: String, proxy: &tao::event_loop::EventLoopProxy<UiEvent>) {
    struct Answer(Option<tao::event_loop::EventLoopProxy<UiEvent>>);
    impl Answer {
        fn send(&mut self, answer: Result<bool, String>) {
            if let Some(p) = self.0.take() {
                let _ = p.send_event(UiEvent::LeaveDecided(answer));
            }
        }
    }
    impl Drop for Answer {
        fn drop(&mut self) {
            self.send(Err("the question ended without an answer".into()));
        }
    }
    let mut answer = Answer(Some(proxy.clone()));
    std::thread::spawn(move || {
        let a = app::ask_checked(&question, &["Cancel", "Go on"], "Cancel").map(|a| a.as_deref() == Some("Go on")).map_err(|e| format!("{e:#}"));
        answer.send(a);
    });
}

/// Do what the leave's step says.
fn act(step: Step, l: &mut Leaving, win: &mut Option<crate::window::EchoWindow>, stop: &Arc<AtomicBool>, ui: &mut Option<Ui>, proxy: &tao::event_loop::EventLoopProxy<UiEvent>) {
    match step {
        Step::Nothing => {}
        Step::Save { n, close } => {
            if let Some(w) = win {
                let _ = w.webview.evaluate_script(&format!(
                    "window.echo && window.echo.prepareClose ? window.echo.prepareClose({n}, {close}) : window.ipc.postMessage(JSON.stringify({{ cmd: 'close_ready', ok: true, n: {n} }}))"
                ));
            }
            let p = proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(5));
                let _ = p.send_event(UiEvent::LeaveTimeout(n));
            });
        }
        Step::Go(then) => l.went(go(then, win, stop, ui)),
        Step::Ask(question) => ask_to_leave(question, proxy),
        Step::Stay(then, why) => {
            thaw(win);
            match then {
                Then::QuitAsked => app::quit_cancelled(&why),
                Then::Update(_) => app::notify("Update not installed", "Notes typed in the Echo window couldn't be saved. Install it from the menu once they are."),
                Then::ChangeHome(_) => app::notify("Data folder not changed", "Notes typed in the Echo window couldn't be saved."),
                Then::CloseWindow | Then::Quit => {}
            }
            #[cfg(target_os = "macos")]
            if matches!(then, Then::Quit | Then::QuitAsked) {
                terminate::cancelled();
            }
        }
        Step::CloseWindow => {
            *win = None;
            apply_dock(false);
        }
    }
    if l.asking_quit() != l.asking_file {
        l.asking_file = l.asking_quit();
        app::asking_to_quit(l.asking_file);
    }
    // A leave that waited for that answer: its turn now.
    if let Some(next) = l.next() {
        let step = l.start(next, win.is_some());
        act(step, l, win, stop, ui, proxy);
    }
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

/// Is a menu open right now? macOS tracks an open menu in the event-tracking run loop mode, and
/// the event loop keeps running meanwhile: a menu replaced then closes under the pointer.
fn menu_open() -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::NSRunLoop;
        NSRunLoop::mainRunLoop().currentMode().is_some_and(|m| m.to_string() == "NSEventTrackingRunLoopMode")
    }
    #[cfg(not(target_os = "macos"))]
    false
}

fn status_text(state: &UiState) -> String {
    match state {
        UiState::Idle { in_call: Some(true) } => "Teams call in progress — not recording".into(),
        UiState::Idle { .. } => "Not recording".into(),
        UiState::Armed => "Armed — recording starts with the next call".into(),
        UiState::Recording { since, memo: true, .. } => format!("● Recording a memo · since {}", since.format("%H:%M")),
        UiState::Recording { title, since, .. } => format!(
            "● Recording{} · since {}",
            title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default(),
            since.format("%H:%M")
        ),
    }
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
    minutes_page: bool,
    auto_minutes: bool,
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
    /// AI profiles (`ai.rs`); None = ai.json unreadable
    ai: Option<AiMenu>,
    has_claude: bool,
    plugin_installed: bool,
    cli_installed: bool,
    /// The transcription server this computer uses ("<name> (<url>)").
    server: Option<String>,
    /// This computer transcribes for others: the URL they connect to.
    serving: Option<String>,
    update: Option<String>,
    calendar_account: Option<String>,
    graph_client: (String, bool),
}

/// The "AI for calls" menu: live notes on, the default id, [(id, name, kind, sign-in line, the
/// account to offer — `claude` profiles signed in to another one than they were set up with —,
/// the model that writes its minutes)].
type AiMenu = (bool, String, Vec<(String, String, crate::ai::Kind, String, Option<String>, String)>);

/// A Claude profile's sign-in, in a menu line (`signin::state`).
fn sign_in_line(p: &crate::ai::Profile, st: Option<&crate::signin::SignInState>) -> String {
    use crate::signin::SignInState as S;
    let what = match st {
        Some(S::SignedIn { account, .. }) => account.clone(),
        Some(S::OtherAccount { account, expected }) => format!("signed in as {account}, set up for {expected}"),
        Some(S::SignedOut) => "not signed in".into(),
        Some(S::Starting | S::Checking) => "checking…".into(),
        Some(S::Waiting { .. }) => "waiting for you in the browser…".into(),
        Some(S::Stopping) => "stopping the sign-in…".into(),
        Some(S::Elsewhere) => "being signed in in a terminal…".into(),
        Some(S::Failed { error }) => format!("the sign-in didn't finish: {error}"),
        Some(S::Unknown { error: Some(e) }) => format!("couldn't check the sign-in: {e}"),
        Some(S::Unknown { error: None }) | None => p.account.clone().map_or_else(|| "not checked yet".into(), |a| format!("{a} (not checked yet)")),
    };
    format!("{}: {what}", p.name)
}

const UPCOMING_MAX: usize = 6;
const RECENT_MAX: usize = 5;

fn load_model(health: &[(String, String, Option<String>)]) -> Model {
    let now = Utc::now();
    let today = Local::now().date_naive();
    let mut upcoming: Vec<MeetingContext> = watch::agenda()
        .into_iter()
        .filter(|m| {
            m.recordable().is_ok() && watch::meeting_start(m).is_some_and(|s| {
                s + chrono::Duration::minutes(4) > now && s.with_timezone(&Local).date_naive() == today
            })
        })
        .collect();
    upcoming.sort_by_key(watch::meeting_start);
    upcoming.truncate(UPCOMING_MAX);

    // The calls in the app's one order (`calls::all_calls`: undated folders oldest), newest first below.
    let dirs: Vec<PathBuf> = crate::calls::all_calls();
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
                "✓ minutes"
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
        minutes_page: cfg.minutes_page,
        auto_minutes: cfg.auto_minutes,
        capture_frames: cfg.capture_frames,
        active_model: cfg.model.clone(),
        downloaded_models,
        languages: cfg.languages.clone(),
        health: health.to_vec(),
        home: paths::home(),
        autostart: app::autostart_enabled(),
        recommended: models::recommend(&models::hardware()).0,
        downloading: models::downloading(),
        notes_in: cfg.notes_in.clone(),
        ai: crate::ai::Settings::load().ok().map(|s| {
            let all = s
                .all()
                .into_iter()
                .map(|p| {
                    let st = crate::signin::state(&p);
                    let other = match &st {
                        Some(crate::signin::SignInState::OtherAccount { account, .. }) => Some(account.clone()),
                        _ => None,
                    };
                    (p.id.clone(), p.name.clone(), p.kind, sign_in_line(&p, st.as_ref()), other, p.notes_model_label())
                })
                .collect();
            (s.live_notes, s.default_id(), all)
        }),
        has_claude: !claude["claude"].is_null(),
        plugin_installed: claude["plugin_installed"] == true,
        cli_installed: app::cli_installed(),
        #[cfg(feature = "transcribe")]
        server: crate::remote::client_label(),
        #[cfg(not(feature = "transcribe"))]
        server: None,
        #[cfg(feature = "transcribe")]
        serving: Some(crate::remote::serve_settings()).filter(|s| s.on).map(|s| crate::remote::serve_url(&s)),
        #[cfg(not(feature = "transcribe"))]
        serving: None,
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
    /// Record the user alone now (`watch::memo_start`).
    Memo,
    CheckUpdates,
    RecordIn(String),
    Stop,
    ArmMeeting(Box<MeetingContext>),
    JoinMeeting(Box<MeetingContext>),
    Open(PathBuf),
    SetUnscheduled(&'static str),
    SetScheduled(&'static str),
    ToggleKeepAudio,
    ToggleMinutesPage,
    ToggleAutoMinutes,
    ToggleFrames,
    UseModel(&'static str),
    RemoveModel(&'static str),
    NotesAnywhere,
    NotesChoose,
    ConnectClaude,
    /// Claude Code's folder on the PATH of new terminals (Settings → Health).
    ClaudePath,
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
    ToggleLiveNotes,
    DefaultProfile(String),
    SignIn(String),
    /// (profile, the account shown)
    AcceptAccount(String, String),
    Recheck,
    AskPermissions,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    RetestGpu,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    ConnectServer,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    DisconnectServer,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    ToggleServing,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    ShowServing,
    OpenUrl(String),
    ChangeHome,
    ToggleAutostart,
    DownloadModel(&'static str),
    SwitchModel(&'static str),
    Quit,
}

struct Ui {
    proxy: tao::event_loop::EventLoopProxy<UiEvent>,
    quit: MenuItem,
    tray: TrayIcon,
    status: MenuItem,
    record: Submenu,
    memo: MenuItem,
    stop: MenuItem,
    note: MenuItem,
    actions: HashMap<MenuId, Action>,
    state: Option<UiState>,
    /// The taskbar theme the icon was drawn for (Windows: the idle icon follows it).
    dark: bool,
    model: Option<Model>,
    health: Vec<(String, String, Option<String>)>,
    last_refresh: Instant,
    /// A refresh waits for the open menu to close (`menu_open`).
    refresh_owed: bool,
    /// Set by a click: open (or focus) the window, at this call; the event loop does it.
    open_window: Option<Option<String>>,
}

fn local_hm(m: &MeetingContext) -> String {
    watch::meeting_start(m).map(|s| s.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_default()
}

/// Settings → Connect to a transcription server…: its URL, then its API key; checked before
/// it is kept.
#[cfg(feature = "transcribe")]
fn connect_server() {
    let Some(url) = app::ask_text(
        "Transcription server URL (http:// or https://).\n\nAnother Echo shows it under Settings → Show URL and API key…",
        "http://",
        "Next",
    ) else {
        return;
    };
    let Some(key) = app::ask_text("API key for this server:", "", "Connect") else { return };
    match crate::remote::connect(&url, &key) {
        Ok(h) => app::notify("Transcription server", &format!("Calls are transcribed on {} ({}, {}).", h.name, h.model, h.engine)),
        Err(e) => app::notify("Could not connect", &format!("{e:#}")),
    }
}

/// What other computers enter to transcribe here; the dialog's Copy button copies both lines.
#[cfg(feature = "transcribe")]
fn show_serving(s: &crate::remote::ServeSettings) {
    let (url, key) = (crate::remote::serve_url(s), s.key.clone());
    let text = format!(
        "Other computers can transcribe their calls on this one while Echo runs here.\n\n\
         In their Echo: Settings → Connect to a transcription server…\n\nURL:  {url}\nAPI key:  {key}\n\n\
         The audio of each utterance is sent over your network unencrypted. Nothing is kept on this computer."
    );
    if app::message(&text, &["Copy", "OK"]).as_deref() == Some("Copy") {
        crate::window::copy_to_clipboard(&format!("URL: {url}\nAPI key: {key}"));
    }
}

fn ellipsize(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

impl Ui {
    fn new(proxy: tao::event_loop::EventLoopProxy<UiEvent>) -> tray_icon::Result<Self> {
        let (icon, _) = icon_for(&UiState::Idle { in_call: None });
        let builder = TrayIconBuilder::new().with_tooltip("Echo");
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        let tray = builder.build()?;
        let mut ui = Ui {
            proxy,
            quit: MenuItem::new("", true, None),
            tray,
            status: MenuItem::new("", false, None),
            record: Submenu::new("", true),
            memo: MenuItem::new("", false, None),
            stop: MenuItem::new("", false, None),
            note: MenuItem::new("", false, None),
            actions: HashMap::new(),
            state: None,
            dark: taskbar_dark(),
            model: None,
            health: health(),
            last_refresh: Instant::now(),
            refresh_owed: false,
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
        // Record ▸ in the default language (Settings' first) / in another one
        self.record = Submenu::new("Record this call", true);
        let default = model.languages.first().cloned().unwrap_or_else(|| "en".into());
        let first = self.item(&format!("In {}", language_name(&default)), true, Some(Action::Record));
        let _ = self.record.append_items(&[&first, &PredefinedMenuItem::separator()]);
        for code in model.languages.iter().skip(1) {
            let it = self.item(&format!("In {}", language_name(code)), true, Some(Action::RecordIn(code.clone())));
            let _ = self.record.append(&it);
        }
        self.memo = self.item("Record a memo", true, Some(Action::Memo));
        self.stop = self.item("Stop recording", false, Some(Action::Stop));
        self.note = self.item("Add a note…", false, Some(Action::AddNote));
        let sep = PredefinedMenuItem::separator;
        let open = self.item("Open Echo…", true, Some(Action::OpenWindow(None)));
        let _ = menu.append_items(&[&self.status, &sep(), &self.record, &self.memo, &self.stop, &self.note, &sep(), &open, &sep()]);

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
        let langs = if model.languages.is_empty() { "en".to_string() } else { model.languages.join(", ") };
        let _ = settings.append_items(&[&sep(), &MenuItem::new(format!("Languages: {langs}"), false, None)]);
        let profile = self.item("Edit my profile (for minutes)…", true, Some(Action::EditProfile));
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
        // Transcription on another computer (`remote.rs`).
        #[cfg(feature = "transcribe")]
        {
            let _ = settings.append_items(&[&sep(), &MenuItem::new("Transcription server", false, None)]);
            match &model.server {
                Some(label) => {
                    let using = MenuItem::new(format!("   Using {}", ellipsize(label, 44)), false, None);
                    let off = self.item("   Stop using it (transcribe here)", true, Some(Action::DisconnectServer));
                    let change = self.item("   Connect to another server…", true, Some(Action::ConnectServer));
                    let _ = settings.append_items(&[&using, &change, &off]);
                }
                None => {
                    let connect = self.item("   Connect to a transcription server…", true, Some(Action::ConnectServer));
                    let _ = settings.append(&connect);
                }
            }
            let serve = self.check("   Transcribe for other computers", model.serving.is_some(), Action::ToggleServing);
            let _ = settings.append(&serve);
            if model.serving.is_some() {
                let show = self.item("   Show URL and API key…", true, Some(Action::ShowServing));
                let _ = settings.append(&show);
            }
        }

        // Speech models: every catalog entry with Use / Download / Remove.
        let models_menu = Submenu::new("Speech models", true);
        // Whisper models first, then the other engines under their name, then speaker
        // detection (only what this build can run).
        let mut section = models::Engine::Whisper.label();
        for m in models::CATALOG.iter().filter(|m| models::runnable(m)) {
            let this = if m.role == models::Role::Speakers { "Speaker detection" } else { m.engine.label() };
            if this != section {
                section = this;
                let _ = models_menu.append_items(&[&sep(), &MenuItem::new(section, false, None)]);
            }
            let downloaded = model.downloaded_models.contains(&m.name);
            let active = model.active_model.as_deref() == Some(m.name);
            let mark = if active { "✓ " } else if downloaded { "   " } else { "↓ " };
            let sub = Submenu::new(format!("{mark}{} · {} MB", m.name, m.size_mb), true);
            let _ = sub.append(&MenuItem::new(m.summary, false, None));
            if m.name == model.recommended {
                let _ = sub.append(&MenuItem::new("Recommended for this computer", false, None));
            }
            if let (Some(calls), true) = (models::used_for(m.name, model.active_model.as_deref()), downloaded) {
                let _ = sub.append(&MenuItem::new(format!("Also used: for {calls}"), false, None));
            }
            let _ = sub.append(&sep());
            if active {
                let _ = sub.append(&MenuItem::new("In use", false, None));
            } else if downloaded && m.role == models::Role::Speakers {
                let on = MenuItem::new("On: the remote side is split by speaker", false, None);
                let remove = self.item(&format!("Remove ({} MB)…", m.size_mb), true, Some(Action::RemoveModel(m.name)));
                let _ = sub.append_items(&[&on, &remove]);
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

        // Claude Code: the plugin (status line, skills), where Echo runs it for minutes, and the minutes.
        let claude = Submenu::new(if model.plugin_installed { "Claude Code ✓" } else { "Claude Code" }, true);
        if !model.has_claude {
            let _ = claude.append(&MenuItem::new("Claude Code is not installed", false, None));
        } else if model.plugin_installed {
            let _ = claude.append(&MenuItem::new("Echo plugin installed", false, None));
        } else {
            let connect = self.item("Connect Claude Code (install the plugin)", true, Some(Action::ConnectClaude));
            let _ = claude.append(&connect);
        }
        let _ = claude.append_items(&[&sep(), &MenuItem::new("Echo runs Claude Code for minutes in", false, None)]);
        let any = self.check("   The data folder", model.notes_in.is_none(), Action::NotesAnywhere);
        let _ = claude.append(&any);
        if let Some(dir) = &model.notes_in {
            let only = self.check(&format!("   {}", ellipsize(&dir.display().to_string(), 40)), true, Action::NotesChoose);
            let _ = claude.append(&only);
        }
        let choose = self.item("   Choose a folder…", true, Some(Action::NotesChoose));
        let _ = claude.append(&choose);
        let auto = self.check("Write minutes automatically (15 min after a call)", model.auto_minutes, Action::ToggleAutoMinutes);
        let page = self.check("Write a minutes page (HTML file) too", model.minutes_page, Action::ToggleMinutesPage);
        let _ = claude.append_items(&[&sep(), &auto, &page]);

        // AI for calls: which model (and account) writes live notes and minutes (`ai.rs`).
        let ai = Submenu::new("AI for calls", true);
        match &model.ai {
            None => {
                let _ = ai.append(&MenuItem::new("Can't read the AI settings (ai.json)", false, None));
            }
            Some((live, default, profiles)) => {
                let live_item = self.check("Live notes during calls", *live, Action::ToggleLiveNotes);
                let _ = ai.append_items(&[&live_item, &sep(), &MenuItem::new("New calls use", false, None)]);
                // Each with the model that writes its minutes; the default's can be changed here.
                for (id, name, _, _, _, minutes) in profiles {
                    let it = self.check(&ellipsize(&format!("   {name} — minutes: {minutes}"), 70), id == default, Action::DefaultProfile(id.clone()));
                    let _ = ai.append(&it);
                }
                if let Some((_, name, _, _, _, minutes)) = profiles.iter().find(|p| &p.0 == default) {
                    let it = self.item(&ellipsize(&format!("Minutes model for {name}: {minutes}…"), 70), model.has_claude, Some(Action::NotesModel));
                    let _ = ai.append(&it);
                }
                let accounts: Vec<_> = profiles.iter().filter(|p| p.2 == crate::ai::Kind::Claude).collect();
                if !accounts.is_empty() {
                    let _ = ai.append(&sep());
                }
                for (id, _, _, line, other, _) in accounts {
                    let _ = ai.append(&MenuItem::new(ellipsize(line, 70), false, None));
                    let sign = self.item("   Sign in…", true, Some(Action::SignIn(id.clone())));
                    let _ = ai.append(&sign);
                    if let Some(other) = other {
                        let accept = self.item(&ellipsize(&format!("   Use {other}"), 60), true, Some(Action::AcceptAccount(id.clone(), other.clone())));
                        let _ = ai.append(&accept);
                    }
                }
                let _ = ai.append_items(&[&sep(), &MenuItem::new("Add or remove profiles with `echoctl ai`", false, None)]);
            }
        }

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
        // What transcription runs on (Windows: the GPU unless its test failed, `gpu.rs`).
        #[cfg(feature = "transcribe")]
        {
            let what = match &model.server {
                Some(label) => format!("server {label}"),
                None => crate::gpu::engine_status(),
            };
            let engine = MenuItem::new(format!("Speech engine: {what}"), false, None);
            let _ = health.append_items(&[&sep(), &engine]);
            if cfg!(windows) {
                let retest = self.item("Re-test GPU", true, Some(Action::RetestGpu));
                let _ = health.append(&retest);
            }
        }
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
        let _ = menu.append_items(&[&settings, &models_menu, &calendar, &claude, &ai, &health, &sep()]);
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
        let force = force || self.refresh_owed;
        if !force && self.last_refresh.elapsed() < Duration::from_secs(5) {
            return;
        }
        // Rebuilding would close the menu being read: on the first tick after it closes.
        if menu_open() {
            self.refresh_owed = true;
            return;
        }
        self.refresh_owed = false;
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
            UiState::Recording { memo, .. } => {
                self.record.set_enabled(false);
                self.stop.set_text(if *memo { "Stop the memo" } else { "Stop recording" });
                self.stop.set_enabled(true);
            }
        }
        // One recording at a time: a memo starts whenever nothing records.
        self.memo.set_enabled(!matches!(state, UiState::Recording { .. }));
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

    /// "Update to …" (menu, or a click on the "Update available" notification): ask, download and
    /// verify it, then — once the window's notes are saved (`Leaving`) — swap it in and stop.
    fn start_update(&self) {
        let Some(r) = crate::update::AVAILABLE.lock().unwrap().clone() else { return };
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let msg = format!("Install Echo {}?\n\nEcho restarts; nothing is recorded meanwhile.", r.version);
            if app::ask(&msg, &["Later", "Update"], "Update").as_deref() != Some("Update") {
                return;
            }
            match crate::update::prepare(&r) {
                Ok(staged) => drop(proxy.send_event(UiEvent::Leave(Then::Update(staged)))),
                Err(e) => app::notify("Update failed", &format!("{e:#}")),
            }
        });
    }

    /// The Quit item while quitting.
    fn quitting(&mut self) {
        self.quit.set_text("Quitting…");
        self.quit.set_enabled(false);
    }

    fn click(&mut self, id: &MenuId) {
        let Some(action) = self.actions.get(id).cloned() else { return };
        self.run(action)
    }

    /// The flows Settings in the window starts (`window::settings_action`), by name: the same
    /// actions as the menu's, so a dialog, a sign-in or a download runs one way only.
    fn page_action(name: &str, arg: &str) -> Option<Action> {
        let model = || models::find(arg).ok().map(|m| m.name);
        Some(match name {
            "change_home" => Action::ChangeHome,
            "install_cli" => Action::InstallCli,
            "connect_claude" => Action::ConnectClaude,
            "claude_path" => Action::ClaudePath,
            "notes_choose" => Action::NotesChoose,
            "notes_anywhere" => Action::NotesAnywhere,
            "check_updates" => Action::CheckUpdates,
            "update" => Action::Update,
            "calendar_login" => Action::CalendarLogin,
            "calendar_logout" => Action::CalendarLogout,
            "calendar_sync" => Action::CalendarSync,
            "calendar_client_id" => Action::CalendarClientId,
            "use_model" => Action::UseModel(model()?),
            "remove_model" => Action::RemoveModel(model()?),
            "download_model" => Action::DownloadModel(model()?),
            "switch_model" => Action::SwitchModel(model()?),
            "edit_profile" => Action::EditProfile,
            "notes_model" => Action::NotesModel,
            "sign_in" => Action::SignIn(arg.to_string()),
            "ask_permissions" => Action::AskPermissions,
            "recheck" => Action::Recheck,
            "open_settings_url" => Action::OpenUrl(arg.to_string()),
            #[cfg(feature = "transcribe")]
            "connect_server" => Action::ConnectServer,
            #[cfg(feature = "transcribe")]
            "disconnect_server" => Action::DisconnectServer,
            #[cfg(feature = "transcribe")]
            "toggle_serving" => Action::ToggleServing,
            #[cfg(feature = "transcribe")]
            "retest_gpu" => Action::RetestGpu,
            _ => return None,
        })
    }

    fn run(&mut self, action: Action) {
        match action {
            Action::Record => drop(watch::arm(None, None, None, None)),
            Action::Memo => {
                if let Err(e) = watch::memo_start(None, None) {
                    app::notify("Memo not started", &format!("{e:#}"));
                }
            }
            Action::RecordIn(code) => drop(watch::arm(None, None, Some(vec![code]), None)),
            Action::Stop => drop(watch::stop()),
            Action::ArmMeeting(m) => drop(watch::arm(m.subject.clone(), Some(*m), None, None)),
            Action::JoinMeeting(m) => drop(watch::join(*m)),
            Action::Open(p) => app::open_path(&p),
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
            Action::ToggleMinutesPage => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.minutes_page = !cfg.minutes_page;
                let _ = cfg.save();
            }
            Action::ToggleAutoMinutes => {
                let saved = Config::load().and_then(|mut cfg| {
                    let on = !cfg.auto_minutes;
                    cfg.set_auto_minutes(on);
                    cfg.save()
                });
                if let Err(e) = saved {
                    app::notify("Couldn't change automatic minutes", &format!("{e:#}"));
                }
            }
            Action::UseModel(name) => drop(models::use_model(name)),
            Action::RemoveModel(name) => app::remove_model_asking(name),
            Action::NotesAnywhere => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.notes_in = None;
                let _ = cfg.save();
            }
            Action::NotesChoose => {
                std::thread::spawn(|| {
                    if let Some(dir) = app::choose_folder("Echo runs Claude Code for call minutes in this folder (its .claude/settings.json applies)") {
                        let Ok(mut cfg) = Config::load() else { return };
                        cfg.notes_in = Some(dunce::canonicalize(&dir).unwrap_or(dir));
                        let _ = cfg.save();
                    }
                });
            }
            Action::ConnectClaude => {
                std::thread::spawn(|| {
                    match app::connect_claude("project", None) {
                        Ok(()) => app::notify("Claude Code connected", "Claude Code lists Echo's plugin in Echo's data folder: its sessions get the status line and /echo:process-call."),
                        Err(e) => app::notify("Could not connect Claude Code", &format!("{e:#}")),
                    }
                    crate::claude_setup::status(true);
                });
            }
            Action::ClaudePath => {
                std::thread::spawn(|| {
                    match crate::claude_setup::add_to_path() {
                        Ok(what) => app::notify("Claude Code is on your PATH", &format!("{what} Open a new terminal to use `claude`.")),
                        Err(e) => app::notify("Couldn't add Claude Code to your PATH", &format!("{e:#}")),
                    }
                    crate::claude_setup::status(true);
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
            Action::ToggleLiveNotes => {
                let r = crate::ai::Settings::update(|s| {
                    s.live_notes = !s.live_notes;
                    Ok(())
                });
                if let Err(e) = r {
                    app::notify("Couldn't change the AI settings", &format!("{e:#}"));
                }
            }
            Action::DefaultProfile(id) => {
                // `update` refuses a default that was removed meanwhile (the menu was open).
                let r = crate::ai::Settings::update(|s| {
                    s.default = (id != crate::ai::SYSTEM).then_some(id);
                    Ok(())
                });
                if let Err(e) = r {
                    app::notify("Couldn't change the AI settings", &format!("{e:#}"));
                }
            }
            Action::AcceptAccount(id, shown) => {
                crate::signin::accept(&id, &shown); // checks off the menu's thread
                return;
            }
            Action::SignIn(id) => {
                crate::signin::start(&id); // the browser opens; Echo notices when it's done
                return;
            }
            Action::NotesModel => {
                // The model that writes the minutes of the profile new calls use (`ai::set_notes_model`).
                std::thread::spawn(|| {
                    let p = crate::ai::Settings::load().and_then(|s| s.get(&s.default_id()));
                    let p = match p {
                        Ok(p) => p,
                        Err(e) => return app::notify("Echo", &format!("The minutes model can't be changed: {e:#}")),
                    };
                    let (current, how) = match p.kind {
                        crate::ai::Kind::System => (
                            Config::load().unwrap_or_default().notes_model_label(),
                            "sonnet, opus or haiku, a full model name, or \"default\" to use Claude Code's own setting (local models, a gateway, Bedrock or Vertex).",
                        ),
                        crate::ai::Kind::Claude => (p.notes_model_label(), "sonnet, opus or haiku, or a full model name; empty for sonnet."),
                        crate::ai::Kind::Local => (p.notes_model_label(), "the model's name on the server; empty for its live model."),
                    };
                    let msg = format!("Model that writes the minutes of {} (the profile new calls use):\n\n{how}", p.name);
                    if let Some(m) = app::ask_text(&msg, &current, "Save") {
                        if let Err(e) = crate::ai::set_notes_model(&p.id, &m) {
                            app::notify("Echo", &format!("The minutes model wasn't changed: {e:#}"));
                        }
                    }
                });
                return;
            }
            Action::OpenWindow(call) => {
                self.open_window = Some(call);
                return;
            }
            Action::AddNote => {
                // Your own note: it becomes the backbone of the call's minutes ("# …" = heading).
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
                    app::open_path(&p);
                }
            }
            Action::InstallCli => {
                std::thread::spawn(|| match app::install_cli() {
                    Ok(()) => app::notify("Command-line tool installed", "Type `echoctl` in a new terminal."),
                    Err(e) => app::notify("Command-line tool not installed", &format!("{e:#}")),
                });
            }
            Action::Recheck => {
                self.health = health();
                crate::claude_setup::status(true);
            }
            Action::AskPermissions => app::request_permissions(),
            Action::ConnectServer => {
                #[cfg(feature = "transcribe")]
                std::thread::spawn(connect_server);
                return;
            }
            Action::DisconnectServer => {
                #[cfg(feature = "transcribe")]
                if let Err(e) = crate::remote::disconnect() {
                    app::notify("Transcription server", &format!("{e:#}"));
                }
            }
            Action::ToggleServing => {
                #[cfg(feature = "transcribe")]
                {
                    let on = !crate::remote::serve_settings().on;
                    match crate::remote::set_serving(on, false) {
                        Ok(s) if s.on => drop(std::thread::spawn(move || show_serving(&s))),
                        Ok(_) => {}
                        Err(e) => app::notify("Transcription server", &format!("{e:#}")),
                    }
                }
            }
            Action::ShowServing => {
                #[cfg(feature = "transcribe")]
                std::thread::spawn(|| show_serving(&crate::remote::serve_settings()));
                return;
            }
            Action::RetestGpu => {
                #[cfg(feature = "transcribe")]
                std::thread::spawn(|| {
                    let ok = crate::gpu::retest();
                    app::notify("GPU test", if ok { "Transcription runs on the GPU." } else { "The GPU test failed: transcription runs on the CPU." });
                });
                return;
            }
            Action::DownloadModel(name) => models::download_in_background(name),
            Action::SwitchModel(name) => models::switch_in_background(name),
            Action::ToggleAutostart => drop(app::set_autostart(!app::autostart_enabled())),
            Action::ChangeHome => {
                // The picker blocks: run it off the event loop, then restart on the new folder once
                // the window's notes are saved (`Leaving`).
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let Some(dir) = app::choose_folder("Folder for Echo calls and minutes") else { return };
                    if dir != paths::home() {
                        let _ = proxy.send_event(UiEvent::Leave(Then::ChangeHome(dir)));
                    }
                });
                return;
            }
            Action::OpenUrl(u) => drop(permissions::open_settings(&u)),
            Action::Quit => {
                let _ = self.proxy.send_event(UiEvent::Leave(Then::Quit));
                return;
            }
        }
        self.refresh_model(true);
    }
}

/// macOS's own Quit — the Dock icon's menu while the window is open, a logout — asks the app
/// (`applicationShouldTerminate:`), which tao's delegate doesn't answer: the process would end at
/// once, losing what is typed in the window and skipping the recording's close. Answered here
/// "later" (`NSTerminateLater`), and the app quits its own way (`Leaving` → the watcher stops), then
/// answers "yes" (macOS terminates it); a quit cancelled after a failed save is answered "no".
#[cfg(target_os = "macos")]
mod terminate {
    use super::{Then, UiEvent};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    static PROXY: OnceLock<Mutex<tao::event_loop::EventLoopProxy<UiEvent>>> = OnceLock::new();
    /// macOS waits for our answer.
    static PENDING: AtomicBool = AtomicBool::new(false);

    const NS_TERMINATE_NOW: usize = 1;
    const NS_TERMINATE_LATER: usize = 2;

    unsafe extern "C-unwind" fn should_terminate(_this: &objc2::runtime::AnyObject, _cmd: objc2::runtime::Sel, _app: &objc2::runtime::AnyObject) -> usize {
        let Some(p) = PROXY.get() else { return NS_TERMINATE_NOW };
        if !PENDING.swap(true, Ordering::SeqCst) {
            let _ = p.lock().unwrap_or_else(|e| e.into_inner()).send_event(UiEvent::Leave(Then::Quit));
        }
        NS_TERMINATE_LATER
    }

    /// Answer `applicationShouldTerminate:` for tao's app delegate (it has no answer of its own).
    pub fn guard(proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
        let _ = PROXY.set(Mutex::new(proxy));
        let Some(cls) = objc2::runtime::AnyClass::get(c"TaoAppDelegateParent") else {
            eprintln!("macOS's Quit doesn't wait for the window's notes: tao's app delegate wasn't found");
            return;
        };
        let sel = objc2::sel!(applicationShouldTerminate:);
        if cls.instance_method(sel).is_some() {
            eprintln!("macOS's Quit doesn't wait for the window's notes: tao answers applicationShouldTerminate: itself");
            return;
        }
        // SAFETY: the function has the selector's signature (`- (NSApplicationTerminateReply)
        // applicationShouldTerminate:(NSApplication *)sender`, type "Q@:@"), and the class exists.
        unsafe {
            let imp: objc2::runtime::Imp = std::mem::transmute(should_terminate as unsafe extern "C-unwind" fn(_, _, _) -> usize);
            objc2::ffi::class_addMethod(cls as *const _ as *mut _, sel, imp, c"Q@:@".as_ptr());
        }
    }

    /// A quit was cancelled (the user kept Echo, `Step::Stay`): so is the one macOS asked for, if it
    /// waits — it asks nothing again until it gets an answer. A quit of `echoctl app quit` may carry
    /// it (`takes_over`).
    pub fn cancelled() {
        reply(false);
    }

    /// Echo has stopped its own way (`Leaving`, the watcher's close): macOS may terminate it now — it
    /// waits for this answer, and the event loop's stop alone doesn't release it.
    pub fn finished() {
        reply(true);
    }

    /// On the event loop's (main) thread only: elsewhere it would take the answer without giving it.
    fn reply(go: bool) {
        let Some(mtm) = objc2_foundation::MainThreadMarker::new() else { return };
        if PENDING.swap(false, Ordering::SeqCst) {
            objc2_app_kit::NSApplication::sharedApplication(mtm).replyToApplicationShouldTerminate(go);
        }
    }
}

/// Do what the leave was for; whether Echo stops now.
fn go(then: Then, win: &mut Option<crate::window::EchoWindow>, stop: &Arc<AtomicBool>, ui: &mut Option<Ui>) -> bool {
    match then {
        Then::CloseWindow => {
            *win = None; // closing the window leaves Echo running, in the menu bar (and the Dock unless hidden)
            apply_dock(false);
            false
        }
        Then::Quit | Then::QuitAsked => {
            if let Some(u) = ui.as_mut() {
                u.quitting();
            }
            stop.store(true, Ordering::SeqCst);
            true
        }
        Then::Update(staged) => match crate::update::commit(&staged, std::process::id()) {
            Ok(()) => {
                stop.store(true, Ordering::SeqCst); // the swap runs after exit
                true
            }
            Err(e) => {
                app::notify("Update not installed", &format!("{e:#}"));
                thaw(win);
                false
            }
        },
        Then::ChangeHome(dir) => match paths::set_home(&dir) {
            Ok(_) => {
                app::RESTART.store(true, Ordering::SeqCst);
                stop.store(true, Ordering::SeqCst);
                true
            }
            Err(e) => {
                app::notify("Couldn't change the data folder", &format!("{e:#}"));
                thaw(win);
                false
            }
        },
    }
}

/// The window's editors were frozen for a leave that didn't happen: writable again.
fn thaw(win: &Option<crate::window::EchoWindow>) {
    if let Some(w) = win {
        let _ = w.webview.evaluate_script("window.echo && window.echo.thaw && window.echo.thaw()");
    }
}

/// Settings → General → "Hide from the Dock when the window is closed".
#[cfg(target_os = "macos")]
fn dock_hidden() -> bool {
    Config::load().is_ok_and(|c| c.hide_dock_icon)
}

/// macOS: the Dock icon as the setting and the window say — shown while the window is open, else unless hidden.
/// On the event loop's (main) thread; elsewhere it does nothing.
fn apply_dock(window_open: bool) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        let Some(mtm) = objc2_foundation::MainThreadMarker::new() else { return };
        let policy = if window_open || !dock_hidden() { NSApplicationActivationPolicy::Regular } else { NSApplicationActivationPolicy::Accessory };
        let app = NSApplication::sharedApplication(mtm);
        if app.activationPolicy() != policy {
            app.setActivationPolicy(policy);
            // Back in the Dock for a window just opened: AppKit doesn't make it the active app by itself (its window
            // stayed behind the one in front), so it is activated as a Dock app's window would be.
            if policy == NSApplicationActivationPolicy::Regular && window_open {
                #[allow(deprecated)]
                app.activateIgnoringOtherApps(true);
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window_open;
}

/// Open the Echo window, or bring it forward (on `call` when one is named).
fn show_window(target: &tao::event_loop::EventLoopWindowTarget<UiEvent>, win: &mut Option<crate::window::EchoWindow>, call: Option<&str>, proxy: &tao::event_loop::EventLoopProxy<UiEvent>) {
    match win {
        Some(w) => {
            match call {
                Some(l) if l.starts_with("echo://") => w.open_link(l),
                Some(c) => w.show_call(c),
                None => {}
            }
            w.window.set_visible(true);
            w.window.set_minimized(false);
            w.window.set_focus();
        }
        None => {
            let p = proxy.clone();
            match crate::window::open(target, call, move |m| drop(p.send_event(UiEvent::Ipc(m)))) {
                Ok(w) => {
                    apply_dock(true);
                    w.window.set_focus();
                    *win = Some(w);
                }
                Err(e) => app::notify("Could not open the Echo window", &e.to_string()),
            }
        }
    }
}

/// The onetest logo as the Dock icon of a dev build, which has no bundle icon. Echo.app keeps
/// macOS: the app's menu bar — Echo (Hide, Hide Others, Quit) and Edit (Undo, Redo, Cut, Copy, Paste, Select All). WebKit
/// takes ⌘C, ⌘V, ⌘X, ⌘A and ⌘Z only from a menu: without one, nothing in the window could be copied or pasted. Quit
/// goes through `applicationShouldTerminate:` as ⌘Q always did (`terminate`).
#[cfg(target_os = "macos")]
fn app_menu() -> Option<Menu> {
    let m = Menu::new();
    let app = Submenu::with_items("Echo", true, &[&PredefinedMenuItem::hide(None), &PredefinedMenuItem::hide_others(None), &PredefinedMenuItem::separator(), &PredefinedMenuItem::quit(None)]).ok()?;
    let edit = Submenu::with_items(
        "Edit",
        true,
        &[&PredefinedMenuItem::undo(None), &PredefinedMenuItem::redo(None), &PredefinedMenuItem::separator(), &PredefinedMenuItem::cut(None), &PredefinedMenuItem::copy(None), &PredefinedMenuItem::paste(None), &PredefinedMenuItem::select_all(None)],
    )
    .ok()?;
    m.append_items(&[&app, &edit]).ok()?;
    m.init_for_nsapp();
    Some(m)
}

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
        // A Dock icon for as long as Echo runs, not only the menu bar's: macOS hides menu-bar icons
        // that don't fit (beside the notch, many of them), and Echo then looked gone. Its click opens
        // the window. Echo.app stays `LSUIElement`, so the CLI from the same binary never shows one.
        // Settings → General → Hide from the Dock (off by default): only while the window is open.
        event_loop.set_activation_policy(if dock_hidden() { ActivationPolicy::Accessory } else { ActivationPolicy::Regular });
        event_loop.set_activate_ignoring_other_apps(false); // starting at login never takes the focus
        crate::app::notifications::init();
    }

    let proxy = event_loop.create_proxy();
    // macOS's own Quit (the Dock's) goes through `Leaving` too.
    #[cfg(target_os = "macos")]
    terminate::guard(proxy.clone());
    let p = proxy.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = p.send_event(UiEvent::Menu(e.id));
    }));
    let p = proxy.clone();
    std::thread::spawn(move || {
        // `Quit` ends the event loop once the watcher has finished — also if it panics: a quit
        // macOS waits on is answered only then.
        struct Finished(tao::event_loop::EventLoopProxy<UiEvent>);
        impl Drop for Finished {
            fn drop(&mut self) {
                let _ = self.0.send_event(UiEvent::Quit);
            }
        }
        let _finished = Finished(p.clone());
        if let Err(e) = watch::run_loop(app, stop, |s| {
            let _ = p.send_event(UiEvent::State(s));
        }) {
            eprintln!("watcher failed: {e:#}");
        }
    });

    let mut ui: Option<Ui> = None;
    // macOS: the app's own menu bar (Echo, Edit), kept for as long as the loop runs.
    #[cfg(target_os = "macos")]
    let mut _app_menu: Option<Menu> = None;
    let mut win: Option<crate::window::EchoWindow> = None;
    let mut leaving = Leaving::default();
    app::UI_LOOP.store(true, Ordering::SeqCst);
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. }
                if win.as_ref().is_some_and(|w| w.window.id() == window_id) =>
            {
                let step = leaving.start(Then::CloseWindow, win.is_some());
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            #[cfg(windows)] // Echo's own title bar: the maximise button shows restore while the window is maximised.
            Event::WindowEvent { window_id, event: WindowEvent::Resized(_), .. } => win.iter().filter(|w| w.window.id() == window_id).for_each(|w| w.report_maximized()),
            Event::UserEvent(UiEvent::Leave(then)) => {
                let step = leaving.start(then, win.is_some());
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            Event::UserEvent(UiEvent::LeaveTimeout(n)) => {
                let step = leaving.answered(n, None);
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            Event::UserEvent(UiEvent::LeaveDecided(answer)) => {
                if let Err(why) = &answer {
                    app::notify("Echo stays open", &format!("Notes typed in the Echo window couldn't be saved, and Echo couldn't ask what to do: {why}"));
                }
                let step = leaving.decided(answer);
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            Event::UserEvent(UiEvent::Ipc(msg)) => match crate::window::closing(&msg) {
                // The page saved what it holds (or couldn't): go on, or ask — a close the page
                // itself explains (its Try again / Close without saving).
                Some(crate::window::Closing::Ready { ok, n }) => {
                    let step = leaving.answered(n, Some(ok));
                    act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
                }
                Some(crate::window::Closing::Now) => {
                    let step = leaving.close_now();
                    act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
                }
                Some(crate::window::Closing::Request) => {
                    let step = leaving.start(Then::CloseWindow, win.is_some());
                    act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
                }
                // Settings in the window started a flow the menu has too: run it the same way.
                None if crate::window::settings_action(&msg).is_some() => {
                    if let (Some((name, arg)), Some(u)) = (crate::window::settings_action(&msg), ui.as_mut()) {
                        match Ui::page_action(&name, &arg) {
                            Some(a) => {
                                u.run(a);
                                u.refresh_model(true);
                            }
                            None => eprintln!("settings: no action {name:?}"),
                        }
                    }
                    if let Some(w) = &win {
                        w.refresh();
                    }
                }
                // The page's own title bar (macOS; Windows with Echo's own title bar): drag, zoom, minimise, resize.
                None if crate::window::window_cmd(&msg).is_some() => win.iter().for_each(|w| w.page_cmd(&msg)),
                None => {
                    let done = crate::window::handle(&msg);
                    #[cfg(windows)] // Settings → Layout → "Echo's own title bar": the window follows at once.
                    win.iter().for_each(|w| w.follow_settings(&msg));
                    if let (Some(js), Some(w)) = (&done.reply, &win) {
                        let _ = w.webview.evaluate_script(js);
                    }
                    if done.refresh {
                        // The action changed something: refresh the page and the menu.
                        if let Some(w) = &win {
                            w.refresh();
                        }
                        if let Some(u) = ui.as_mut() {
                            u.refresh_model(true);
                        }
                    }
                }
            },
            // The Dock icon clicked (or Echo.app opened again) with no window showing: open it — also
            // when the menu-bar icon is hidden (no room beside the notch) or couldn't be made.
            Event::Reopen { has_visible_windows: false, .. } if leaving.may_open() => show_window(target, &mut win, None, &proxy),
            // macOS: create the icon only once the event loop is running.
            Event::NewEvents(StartCause::Init) => {
                #[cfg(target_os = "macos")]
                {
                    set_app_icon(); // once AppKit has finished launching: earlier it would put its own back
                    if let Some(m) = app_menu() { _app_menu.replace(m); } // held by the loop: the menu bar lasts as long as its `Menu` (`=` reads as never read)
                }
                match Ui::new(proxy.clone()) {
                    Ok(u) => ui = Some(u),
                    Err(e) => eprintln!("tray icon unavailable: {e}"),
                }
            }
            // The system opens an echo:// link (a note in Obsidian, a mail): the window, there.
            Event::Opened { urls } => {
                for u in urls.iter().map(|u| u.to_string()).filter(|u| crate::links::Link::parse(u).is_some()).take(1) {
                    if leaving.may_open() {
                        show_window(target, &mut win, Some(&u), &proxy);
                    }
                }
            }
            Event::UserEvent(UiEvent::State(s)) => {
                // `echoctl app quit` — also when the tray icon couldn't be made (no window then).
                if app::take_quit_request() {
                    let _ = proxy.send_event(UiEvent::Leave(Then::QuitAsked));
                }
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
                    if let Some(call) = u.open_window.take().filter(|_| leaving.may_open()) {
                        show_window(target, &mut win, call.as_deref(), &proxy);
                    }
                }
            }
            Event::UserEvent(UiEvent::Quit) => {
                // Terminals in the window (Claude sessions) end with Echo; they resume by id later.
                crate::terminal::close_all();
                if app::RESTART.load(Ordering::SeqCst) {
                    let _ = app::spawn_app(true);
                }
                // A quit macOS asked for waits for its answer: the watcher has closed everything.
                #[cfg(target_os = "macos")]
                terminate::finished();
                *control_flow = ControlFlow::Exit
            }
            _ => {}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaving_waits_for_the_notes_in_the_window() {
        assert!(matches!(after_handshake(Then::Quit, Some(true)), Next::Go(Then::Quit)));
        assert!(matches!(after_handshake(Then::CloseWindow, Some(false)), Next::Stay), "the page says why, with its own buttons");
        assert!(matches!(after_handshake(Then::QuitAsked, Some(false)), Next::Ask(Then::QuitAsked, _)));
        assert!(matches!(after_handshake(Then::ChangeHome("/x".into()), Some(false)), Next::Ask(Then::ChangeHome(_), _)));
        assert!(matches!(after_handshake(Then::CloseWindow, None), Next::Ask(Then::CloseWindow, _)), "no answer: asked, never closed quietly");
        assert!(matches!(after_handshake(Then::Quit, None), Next::Ask(Then::Quit, _)));
    }

    #[test]
    fn a_quit_is_never_dropped_for_another_leave() {
        let home = || Then::ChangeHome("/x".into());
        assert!(takes_over(&Then::CloseWindow, &Then::Quit) && takes_over(&Then::CloseWindow, &home()));
        assert!(takes_over(&home(), &Then::Quit) && takes_over(&home(), &Then::QuitAsked), "macOS waits for its answer");
        assert!(takes_over(&Then::Quit, &Then::QuitAsked), "`app quit` waits for the outcome");
        for (pending, new) in [(Then::QuitAsked, Then::Quit), (Then::Quit, Then::CloseWindow), (Then::Quit, home()), (home(), Then::CloseWindow)] {
            assert!(!takes_over(&pending, &new), "{pending:?} stays for {new:?}");
        }
    }

    #[test]
    fn one_leave_at_a_time_from_the_save_to_echo_stopping() {
        let mut l = Leaving::default();
        // A close with the window open: its page saves first.
        assert!(matches!(l.start(Then::CloseWindow, true), Step::Save { n: 1, close: true }));
        // A quit meanwhile takes its place; a close then doesn't.
        assert!(matches!(l.start(Then::Quit, true), Step::Nothing));
        assert!(matches!(l.start(Then::CloseWindow, true), Step::Nothing));
        // An answer for another handshake changes nothing; this one's failure asks the user.
        assert!(matches!(l.answered(7, Some(true)), Step::Nothing));
        assert!(matches!(l.answered(1, Some(false)), Step::Ask(q) if q.contains("Quit Echo anyway?")));
        assert!(matches!(l.answered(1, None), Step::Nothing), "the answered handshake's timeout");
        // While the user is asked: no second handshake; `app quit` waits for the answer (and the
        // CLI with it: quit.asking), it doesn't take this question over.
        assert!(!l.asking_quit());
        assert!(matches!(l.start(Then::QuitAsked, true), Step::Nothing));
        assert!(l.asking_quit() && l.next().is_none());
        // Kept open: then `app quit` gets its own turn — its own save and question.
        assert!(matches!(l.decided(Ok(false)), Step::Stay(Then::Quit, why) if why.contains("kept open")));
        let next = l.next();
        assert!(matches!(next, Some(Then::QuitAsked)) && !l.asking_quit());
        assert!(matches!(l.start(next.unwrap(), true), Step::Save { n: 2, close: false }));
        assert!(matches!(l.answered(2, Some(false)), Step::Ask(_)));
        assert!(l.asking_quit());
        assert!(matches!(l.decided(Ok(false)), Step::Stay(Then::QuitAsked, _)));
        // A folder change that goes ahead: Echo stops, and nothing else starts.
        assert!(matches!(l.start(Then::ChangeHome("/x".into()), true), Step::Save { n: 3, close: false }));
        assert!(matches!(l.answered(3, Some(true)), Step::Go(Then::ChangeHome(_))));
        l.went(true);
        assert!(matches!(l.start(Then::Quit, true), Step::Nothing));
        assert!(matches!(l.close_now(), Step::Nothing));
    }

    #[test]
    fn a_question_is_answered_only_for_its_own_leave() {
        // "Close the window anyway?" open; a quit arrives: it waits, the answer is the close's.
        let mut l = Leaving::default();
        l.start(Then::CloseWindow, true);
        assert!(matches!(l.answered(1, None), Step::Ask(q) if q.contains("Close the window anyway?")));
        l.start(Then::Quit, true);
        assert!(matches!(l.decided(Ok(true)), Step::Go(Then::CloseWindow)));
        l.went(false);
        assert!(matches!(l.next(), Some(Then::Quit)), "then the quit goes on (no window: at once)");
        // Only one waits: what would take over the waiting one replaces it, else it's dropped.
        let mut l = Leaving::default();
        l.start(Then::ChangeHome("/x".into()), true);
        l.answered(1, None);
        l.start(Then::Quit, true);
        l.start(Then::CloseWindow, true);
        l.start(Then::QuitAsked, true);
        l.decided(Ok(false));
        assert!(matches!(l.next(), Some(Then::QuitAsked)));
    }

    #[test]
    fn without_a_window_a_leave_goes_at_once_and_close_without_saving_goes_on() {
        let mut l = Leaving::default();
        assert!(matches!(l.start(Then::QuitAsked, false), Step::Go(Then::QuitAsked)), "no window (or no tray icon): nothing to save");
        let mut l = Leaving::default();
        assert!(matches!(l.close_now(), Step::Go(Then::CloseWindow)));
        l.went(false);
        // Still saving: the leave goes on without the notes.
        assert!(matches!(l.start(Then::ChangeHome("/x".into()), true), Step::Save { .. }));
        assert!(matches!(l.close_now(), Step::Go(Then::ChangeHome(_))));
        l.went(false); // the folder couldn't be changed: Echo stays
        // Asking the user: the window closes, the question stays.
        assert!(matches!(l.start(Then::Quit, true), Step::Save { n: 2, .. }));
        assert!(matches!(l.answered(2, None), Step::Ask(_)));
        assert!(matches!(l.close_now(), Step::CloseWindow));
        assert!(matches!(l.decided(Err("no dialog".into())), Step::Stay(Then::Quit, why) if why.contains("couldn't ask")));
    }
}
