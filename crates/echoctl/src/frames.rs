//! Key frames of the meeting window (macOS): what was on screen — shared slides, documents,
//! demos — saved as JPEGs while a call is recorded, so the notes can use them.
//!
//! Every few seconds the Teams meeting window is captured (`screencapture -l <window>`, which
//! works while the window is behind others, not when minimized). A frame is kept only when it
//! has settled (barely differs from the previous capture) and differs clearly from the last
//! kept frame — on a 64×36 grayscale thumbnail, so this is cheap. Kept frames go to
//! `<call>/frames/<mm>m<ss>s.jpg`, indexed in `<call>/frames.jsonl` (`{t, file, change}`).
//!
//! Needs the Screen Recording permission (the full one: "System Audio Recording Only" does not
//! cover window captures). Off by default: `config.capture_frames`.

use std::ffi::c_void;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use image::imageops::FilterType;
use image::GrayImage;
use objc2::rc::Retained;
use objc2_foundation::NSString;
use serde_json::json;

/// How often the window is captured.
pub const POLL: Duration = Duration::from_secs(3);
const THUMB_W: u32 = 64;
const THUMB_H: u32 = 36;
/// A thumbnail cell counts as changed when its brightness moved by more than this.
const CELL_DELTA: i16 = 24;
/// Kept: differs from the last kept frame in ≥ this share of cells…
const NEW_CONTENT: f32 = 0.12;
/// …and has settled: differs from the previous capture in < this share (not mid-animation).
const SETTLED: f32 = 0.03;
const MIN_GAP: Duration = Duration::from_secs(8);
/// Captures before this point are the join screen / lobby.
const SKIP_FIRST: Duration = Duration::from_secs(5);
const MAX_FRAMES: usize = 150;
const MAX_WIDTH: u32 = 1600;

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
fn meeting_window(pid: i32) -> Option<u32> {
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

fn changed_share(a: &GrayImage, b: &GrayImage) -> f32 {
    let changed = a.pixels().zip(b.pixels()).filter(|(x, y)| (x.0[0] as i16 - y.0[0] as i16).abs() > CELL_DELTA).count();
    changed as f32 / (THUMB_W * THUMB_H) as f32
}

/// Decides which captures are key frames. Separate from capturing so it can be tested.
#[derive(Default)]
pub struct KeyFramePicker {
    previous: Option<GrayImage>,
    kept: Option<GrayImage>,
    kept_at: Option<Duration>,
    pub count: usize,
}

impl KeyFramePicker {
    /// Some(change vs the last kept frame) when this capture should be kept.
    pub fn consider(&mut self, thumb: GrayImage, at: Duration) -> Option<f32> {
        let settled = self.previous.as_ref().is_some_and(|p| changed_share(p, &thumb) < SETTLED);
        self.previous = Some(thumb.clone());
        if at < SKIP_FIRST || self.count >= MAX_FRAMES || !settled {
            return None;
        }
        let change = self.kept.as_ref().map(|k| changed_share(k, &thumb)).unwrap_or(1.0);
        let gap_ok = self.kept_at.is_none_or(|t| at.saturating_sub(t) >= MIN_GAP);
        if change >= NEW_CONTENT && gap_ok {
            self.kept = Some(thumb);
            self.kept_at = Some(at);
            self.count += 1;
            return Some(change);
        }
        None
    }
}

pub struct FrameRecorder {
    pid: i32,
    dir: PathBuf,
    t0: Instant,
    picker: KeyFramePicker,
    index: std::fs::File,
    tmp: PathBuf,
}

impl FrameRecorder {
    /// None if Screen Recording isn't granted or Teams isn't running.
    pub fn teams(dir: &Path, t0: Instant) -> Option<Self> {
        if !permitted() {
            return None;
        }
        let pid = crate::ax::app_pid(crate::ax::TEAMS_EXECUTABLE)?;
        std::fs::create_dir_all(dir.join("frames")).ok()?;
        let index = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("frames.jsonl")).ok()?;
        let tmp = std::env::temp_dir().join(format!("echo-frame-{}.png", std::process::id()));
        Some(FrameRecorder { pid, dir: dir.to_path_buf(), t0, picker: KeyFramePicker::default(), index, tmp })
    }

    /// Capture once; keep it if it's a key frame. False when the per-call cap is reached.
    pub fn poll(&mut self) -> bool {
        if self.picker.count >= MAX_FRAMES {
            return false;
        }
        let at = self.t0.elapsed();
        let Some(window) = meeting_window(self.pid) else { return true };
        let ok = Command::new("screencapture")
            .args(["-x", "-o", "-t", "png", &format!("-l{window}")])
            .arg(&self.tmp)
            .status()
            .is_ok_and(|s| s.success());
        let Some(img) = ok.then(|| image::open(&self.tmp).ok()).flatten() else { return true };
        let thumb = img.resize_exact(THUMB_W, THUMB_H, FilterType::Triangle).to_luma8();
        if let Some(change) = self.picker.consider(thumb, at) {
            let secs = at.as_secs();
            let name = format!("{:02}m{:02}s.jpg", secs / 60, secs % 60);
            let img = if img.width() > MAX_WIDTH { img.resize(MAX_WIDTH, u32::MAX, FilterType::Triangle) } else { img };
            let path = self.dir.join("frames").join(&name);
            if let Ok(f) = std::fs::File::create(&path) {
                let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 80);
                if enc.encode_image(&img.to_rgb8()).is_ok() {
                    let line = json!({ "t": secs, "file": format!("frames/{name}"), "change": (change * 100.0).round() / 100.0 });
                    let _ = writeln!(self.index, "{line}");
                }
            }
        }
        let _ = std::fs::remove_file(&self.tmp);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: u8) -> GrayImage {
        GrayImage::from_pixel(THUMB_W, THUMB_H, image::Luma([v]))
    }

    #[test]
    fn keeps_settled_new_content_only() {
        let mut p = KeyFramePicker::default();
        let s = Duration::from_secs;
        assert!(p.consider(flat(10), s(1)).is_none()); // lobby window
        assert!(p.consider(flat(10), s(6)).is_some()); // first settled frame
        assert!(p.consider(flat(10), s(9)).is_none()); // same content
        assert!(p.consider(flat(200), s(20)).is_none()); // new slide, still changing
        assert!(p.consider(flat(200), s(23)).is_some()); // settled → kept
        assert!(p.consider(flat(90), s(26)).is_none());
        assert!(p.consider(flat(90), s(28)).is_none()); // settled but < 8 s since last kept
        assert!(p.consider(flat(90), s(32)).is_some());
        assert_eq!(p.count, 3);
    }
}
