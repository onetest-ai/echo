//! Key frames of the meeting window: what was on screen — shared slides, documents, demos —
//! saved as JPEGs while a call is recorded, so the notes can use them.
//!
//! Every few seconds the Teams meeting window is captured (macOS: `screencapture -l <window>`;
//! Windows: `PrintWindow` with full content), which works while the window is behind others,
//! not when minimized. Windows captures only while someone else shares their screen (Teams'
//! content pop-out button, `popout-content-button`); otherwise a call's frames would be video
//! tiles. A frame is kept only when it has settled (barely differs from the previous capture)
//! and differs from *every* kept frame — on a small grayscale thumbnail of the whole window
//! (tuned per platform, see below), so a slide shown again is not kept twice. Kept
//! frames go to `<call>/frames/<mm>m<ss>s.jpg`, indexed in `<call>/frames.jsonl`
//! (`{t, file, change, shared_by}`).
//!
//! macOS needs the Screen Recording permission (the full one: "System Audio Recording Only"
//! does not cover window captures); Windows needs none. Off by default: `config.capture_frames`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::imageops::FilterType;
use image::{DynamicImage, GrayImage};
use serde_json::json;

#[cfg(target_os = "macos")]
use self::macos::{capture, Target};
#[cfg(target_os = "macos")]
pub use self::macos::{permitted, request};
#[cfg(windows)]
use self::windows::{capture, Target};

// Two tunings. Windows captures only while someone else shares, so it can afford to be
// sensitive: sized from a live call, the shared slide is ~30 % of the call window, the next
// slide changes ~6–10 % of it, two title slides with different subtitles ~1–2 % (the timer,
// the cursor and speaking indicators < 1 %). macOS captures the whole call (no share
// detection there yet), video tiles included, so it keeps the coarser tuning: 4 % would let
// settled webcam movement fill the frame cap.

/// How often the window is captured.
#[cfg(windows)]
pub const POLL: Duration = Duration::from_secs(2);
#[cfg(not(windows))]
pub const POLL: Duration = Duration::from_secs(3);
#[cfg(windows)]
const THUMB: (u32, u32) = (128, 72);
#[cfg(not(windows))]
const THUMB: (u32, u32) = (64, 36);
const THUMB_W: u32 = THUMB.0;
const THUMB_H: u32 = THUMB.1;
/// A thumbnail cell counts as changed when its brightness moved by more than this.
const CELL_DELTA: i16 = 24;
/// Kept: differs from every kept frame in ≥ this share of cells…
#[cfg(windows)]
const NEW_CONTENT: f32 = 0.04;
#[cfg(not(windows))]
const NEW_CONTENT: f32 = 0.12;
/// …and has settled: differs from the previous capture in < this share (not mid-animation).
const SETTLED: f32 = 0.03;
#[cfg(windows)]
const MIN_GAP: Duration = Duration::from_secs(4);
#[cfg(not(windows))]
const MIN_GAP: Duration = Duration::from_secs(8);
/// Captures before this point are the join screen / lobby.
const SKIP_FIRST: Duration = Duration::from_secs(5);
const MAX_FRAMES: usize = 150;
const MAX_WIDTH: u32 = 1600;

fn changed_share(a: &GrayImage, b: &GrayImage) -> f32 {
    let changed = a.pixels().zip(b.pixels()).filter(|(x, y)| (x.0[0] as i16 - y.0[0] as i16).abs() > CELL_DELTA).count();
    changed as f32 / (THUMB_W * THUMB_H) as f32
}

/// Decides which captures are key frames. Separate from capturing so it can be tested.
#[derive(Default)]
pub struct KeyFramePicker {
    previous: Option<GrayImage>,
    kept: Vec<GrayImage>,
    kept_at: Option<Duration>,
    pub count: usize,
}

impl KeyFramePicker {
    /// Some(change vs the closest kept frame) when this capture should be kept.
    pub fn consider(&mut self, thumb: GrayImage, at: Duration) -> Option<f32> {
        let settled = self.previous.as_ref().is_some_and(|p| changed_share(p, &thumb) < SETTLED);
        self.previous = Some(thumb.clone());
        if at < SKIP_FIRST || self.count >= MAX_FRAMES || !settled {
            return None;
        }
        let change = self.kept.iter().map(|k| changed_share(k, &thumb)).fold(1.0, f32::min);
        let gap_ok = self.kept_at.is_none_or(|t| at.saturating_sub(t) >= MIN_GAP);
        if change >= NEW_CONTENT && gap_ok {
            self.kept.push(thumb);
            self.kept_at = Some(at);
            self.count += 1;
            return Some(change);
        }
        None
    }

    /// A gap in capturing (nobody sharing): the next capture can't be "settled" against one
    /// taken before the gap.
    pub fn pause(&mut self) {
        self.previous = None;
    }
}

/// One capture of the call window, and who was presenting in it if known.
pub struct Capture {
    img: DynamicImage,
    shared_by: Option<String>,
}

/// macOS: the Teams process's largest on-screen window, via `screencapture`.
#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::process::Command;

    use objc2::rc::Retained;
    use objc2_foundation::NSString;

    type CFTypeRef = *const c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CFTypeRef;
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(a: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(a: CFTypeRef, i: isize) -> CFTypeRef;
        fn CFDictionaryGetValue(d: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFNumberGetValue(n: CFTypeRef, kind: isize, out: *mut c_void) -> bool;
        fn CFRelease(cf: CFTypeRef);
    }

    const K_CG_WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1;
    const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP: u32 = 16;
    const K_CF_NUMBER_F64: isize = 13;

    /// Screen Recording granted to this process (the app)?
    pub fn permitted() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    /// Ask for Screen Recording (macOS shows its prompt / sends the user to Settings once).
    pub fn request() -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
    }

    unsafe fn number(dict: CFTypeRef, key: &str) -> Option<f64> {
        let k: Retained<NSString> = NSString::from_str(key);
        let v = CFDictionaryGetValue(dict, Retained::as_ptr(&k).cast());
        if v.is_null() {
            return None;
        }
        let mut d = 0f64;
        CFNumberGetValue(v, K_CF_NUMBER_F64, (&mut d as *mut f64).cast()).then_some(d)
    }

    /// The call app's main window: the largest normal (layer 0) on-screen window of `pid`.
    pub(super) fn meeting_window(pid: i32) -> Option<u32> {
        unsafe {
            let list = CGWindowListCopyWindowInfo(K_CG_WINDOW_LIST_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP, 0);
            if list.is_null() {
                return None;
            }
            let mut best: Option<(f64, u32)> = None;
            for i in 0..CFArrayGetCount(list) {
                let w = CFArrayGetValueAtIndex(list, i);
                if number(w, "kCGWindowOwnerPID") != Some(pid as f64) || number(w, "kCGWindowLayer") != Some(0.0) {
                    continue;
                }
                let k: Retained<NSString> = NSString::from_str("kCGWindowBounds");
                let bounds = CFDictionaryGetValue(w, Retained::as_ptr(&k).cast());
                let area = if bounds.is_null() {
                    0.0
                } else {
                    number(bounds, "Width").unwrap_or(0.0) * number(bounds, "Height").unwrap_or(0.0)
                };
                let id = number(w, "kCGWindowNumber").unwrap_or(0.0) as u32;
                if area > 200.0 * 200.0 && best.is_none_or(|(a, _)| area > a) {
                    best = Some((area, id));
                }
            }
            CFRelease(list);
            best.map(|(_, id)| id)
        }
    }

    /// What to capture: the Teams process + a temp file for `screencapture`.
    pub struct Target {
        pid: i32,
        tmp: PathBuf,
    }

    impl Target {
        pub fn teams() -> Option<Self> {
            if !permitted() {
                return None;
            }
            let pid = crate::ax::app_pid(crate::ax::TEAMS_EXECUTABLE)?;
            let tmp = std::env::temp_dir().join(format!("echo-frame-{}.png", std::process::id()));
            Some(Target { pid, tmp })
        }
    }

    pub fn capture(target: &mut Target) -> Option<super::Capture> {
        let window = meeting_window(target.pid)?;
        let ok = Command::new("screencapture")
            .args(["-x", "-o", "-t", "png", &format!("-l{window}")])
            .arg(&target.tmp)
            .status()
            .is_ok_and(|s| s.success());
        let img = ok.then(|| image::open(&target.tmp).ok()).flatten();
        let _ = std::fs::remove_file(&target.tmp);
        img.map(|img| super::Capture { img, shared_by: None })
    }
}

/// Windows: Teams' call window (the one holding the mic button, found with UI Automation — never
/// the main window, which shows chats), via `PrintWindow`, while someone else shares their
/// screen. No call window or no share, no frame. While you present, the call window shows only
/// a compact view and Teams doesn't say which window you share: nothing is captured.
#[cfg(windows)]
mod windows {
    use image::{DynamicImage, RgbaImage};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic};

    /// Includes DirectComposition content (Chromium/WebView2 would otherwise come out black).
    const PW_RENDERFULLCONTENT: u32 = 2;

    /// Teams' call window, kept between captures.
    pub struct Target(crate::ax::CallWindow);

    impl Target {
        pub fn teams() -> Option<Self> {
            crate::ax::CallWindow::teams().map(Target)
        }
    }

    fn size(h: HWND) -> (i32, i32) {
        let mut r = RECT::default();
        let _ = unsafe { GetWindowRect(h, &mut r) };
        (r.right - r.left, r.bottom - r.top)
    }

    pub fn capture(target: &mut Target) -> Option<super::Capture> {
        let crate::ax::Sharing::Them(who) = target.0.sharing() else { return None };
        let hwnd = target.0.hwnd().filter(|&h| !unsafe { IsIconic(h) }.as_bool())?;
        let (w, h) = size(hwnd);
        if w < 200 || h < 200 {
            return None;
        }
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(dc, bmp.into());
            let ok = PrintWindow(hwnd, dc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut px = vec![0u8; (w * h * 4) as usize];
            let rows = if ok { GetDIBits(dc, bmp, 0, h as u32, Some(px.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS) } else { 0 };
            SelectObject(dc, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
            if rows != h {
                return None;
            }
            for p in px.as_chunks_mut::<4>().0 {
                p.swap(0, 2); // BGRA → RGBA
                p[3] = 255;
            }
            let img = RgbaImage::from_raw(w as u32, h as u32, px).map(DynamicImage::ImageRgba8)?;
            Some(super::Capture { img, shared_by: who })
        }
    }
}

pub struct FrameRecorder {
    target: Target,
    dir: PathBuf,
    t0: Instant,
    picker: KeyFramePicker,
    index: std::fs::File,
}

impl FrameRecorder {
    /// None if Screen Recording isn't granted or Teams isn't running (macOS).
    pub fn teams(dir: &Path, t0: Instant) -> Option<Self> {
        let target = Target::teams()?;
        std::fs::create_dir_all(dir.join("frames")).ok()?;
        let index = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("frames.jsonl")).ok()?;
        Some(FrameRecorder { target, dir: dir.to_path_buf(), t0, picker: KeyFramePicker::default(), index })
    }

    /// Capture once; keep it if it's a key frame. False when the per-call cap is reached.
    pub fn poll(&mut self) -> bool {
        if self.picker.count >= MAX_FRAMES {
            return false;
        }
        let at = self.t0.elapsed();
        let Some(Capture { img, shared_by }) = capture(&mut self.target) else {
            self.picker.pause();
            return true;
        };
        let thumb = img.resize_exact(THUMB_W, THUMB_H, FilterType::Triangle).to_luma8();
        if let Some(change) = self.picker.consider(thumb, at) {
            let secs = at.as_secs();
            let name = format!("{:02}m{:02}s.jpg", secs / 60, secs % 60);
            let img = if img.width() > MAX_WIDTH { img.resize(MAX_WIDTH, u32::MAX, FilterType::Triangle) } else { img };
            let path = self.dir.join("frames").join(&name);
            if let Ok(f) = std::fs::File::create(&path) {
                let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 80);
                if enc.encode_image(&img.to_rgb8()).is_ok() {
                    let line = json!({ "t": secs, "file": format!("frames/{name}"), "change": (change * 100.0).round() / 100.0, "shared_by": shared_by });
                    let _ = writeln!(self.index, "{line}");
                }
            }
        }
        true
    }
}

/// `echoctl frame-grab <file>`: one capture of the Teams window (to check what frames see).
pub fn grab(out: &Path) -> anyhow::Result<()> {
    let mut target = Target::teams().ok_or_else(|| anyhow::anyhow!("Teams is not running, or Screen Recording is not granted"))?;
    let Capture { img, shared_by } = capture(&mut target)
        .ok_or_else(|| anyhow::anyhow!("nothing to capture: no Teams call window, nobody else sharing, or minimized"))?;
    img.to_rgb8().save(out)?;
    println!("{}×{} → {} (shared by {})", img.width(), img.height(), out.display(), shared_by.as_deref().unwrap_or("?"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: u8) -> GrayImage {
        GrayImage::from_pixel(THUMB_W, THUMB_H, image::Luma([v]))
    }

    // Times in these tests follow the platform's MIN_GAP (4 s on Windows, 8 s on macOS).

    #[test]
    fn keeps_settled_new_content_only() {
        let mut p = KeyFramePicker::default();
        let s = Duration::from_secs;
        let g = MIN_GAP.as_secs();
        assert!(p.consider(flat(10), s(1)).is_none()); // lobby window
        assert!(p.consider(flat(10), s(6)).is_some()); // first settled frame
        assert!(p.consider(flat(10), s(8)).is_none()); // same content
        assert!(p.consider(flat(200), s(20)).is_none()); // new slide, still changing
        assert!(p.consider(flat(200), s(22)).is_some()); // settled → kept
        assert!(p.consider(flat(90), s(22 + g - 2)).is_none());
        assert!(p.consider(flat(90), s(22 + g - 1)).is_none()); // settled but < MIN_GAP since last kept
        assert!(p.consider(flat(90), s(22 + g)).is_some());
        assert_eq!(p.count, 3);
    }

    #[test]
    fn a_slide_shown_again_is_not_kept_twice() {
        let mut p = KeyFramePicker::default();
        let s = Duration::from_secs;
        let g = MIN_GAP.as_secs();
        for (v, t) in [(10, 6), (10, 8), (200, 8 + g), (200, 10 + g)] {
            p.consider(flat(v), s(t));
        }
        assert_eq!(p.count, 2);
        // Back to the first slide: settled and far from the last kept one, but seen before.
        assert!(p.consider(flat(10), s(30)).is_none());
        assert!(p.consider(flat(10), s(32)).is_none());
        assert_eq!(p.count, 2);
    }

    #[test]
    fn after_a_pause_the_next_capture_must_settle_again() {
        let mut p = KeyFramePicker::default();
        let s = Duration::from_secs;
        p.consider(flat(10), s(6));
        p.pause(); // nobody sharing for a while
        assert!(p.consider(flat(200), s(40)).is_none()); // first capture after the gap
        assert!(p.consider(flat(200), s(42)).is_some());
    }
}
