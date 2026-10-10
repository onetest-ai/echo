//! macOS TCC checks. Permissions attach to the *responsible* app: Echo.app when the app
//! records (the normal case), or the terminal when the CLI is run by hand — so the guidance
//! names that app explicitly.

use std::ffi::{c_void, CString};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use objc2_foundation::{NSDictionary, NSNumber, NSString};

use super::{Check, Report, Status};

const MIC_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
const AUDIO_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
const AX_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

fn os_version() -> String {
    Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Process taps need macOS 14.2+.
fn os_supported(v: &str) -> bool {
    let mut it = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (major, minor) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    major > 14 || (major == 14 && minor >= 2)
}

fn responsible_app() -> Option<String> {
    if crate::app::bundle_path().is_some() {
        return Some("Echo".into());
    }
    std::env::var("__CFBundleIdentifier")
        .ok()
        .or_else(|| std::env::var("TERM_PROGRAM").ok())
}

fn mic_status() -> Status {
    let Some(media) = (unsafe { AVMediaTypeAudio }) else { return Status::Unknown };
    match unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) } {
        AVAuthorizationStatus::Authorized => Status::Granted,
        AVAuthorizationStatus::NotDetermined => Status::NotDetermined,
        _ => Status::Denied,
    }
}

/// "System Audio Recording" has no public status API. Like other process-tap apps we ask
/// the private TCC framework, and degrade to Unknown if that symbol is unavailable.
type PreflightFn = unsafe extern "C" fn(*const c_void, *const c_void) -> i32;
type RequestFn = unsafe extern "C" fn(*const c_void, *const c_void, *const c_void);

unsafe fn tcc_symbol(name: &str) -> *mut c_void {
    let path = CString::new("/System/Library/PrivateFrameworks/TCC.framework/Versions/A/TCC").unwrap();
    let h = libc::dlopen(path.as_ptr(), libc::RTLD_LAZY);
    if h.is_null() {
        return std::ptr::null_mut();
    }
    libc::dlsym(h, CString::new(name).unwrap().as_ptr())
}

fn audio_capture_service() -> objc2::rc::Retained<NSString> {
    NSString::from_str("kTCCServiceAudioCapture")
}

fn system_audio_status() -> Status {
    unsafe {
        let sym = tcc_symbol("TCCAccessPreflight");
        if sym.is_null() {
            return Status::Unknown;
        }
        let f: PreflightFn = std::mem::transmute(sym);
        let svc = audio_capture_service();
        let raw = f(objc2::rc::Retained::as_ptr(&svc).cast(), std::ptr::null());
        if std::env::var_os("ECHOCTL_DEBUG").is_some() {
            eprintln!("debug: TCCAccessPreflight(kTCCServiceAudioCapture) = {raw}");
        }
        match raw {
            0 => Status::Granted,
            1 => Status::Denied,
            _ => Status::NotDetermined,
        }
    }
}

fn accessibility_status() -> Status {
    if unsafe { AXIsProcessTrusted() } {
        Status::Granted
    } else {
        Status::Denied
    }
}

/// Peak level seen by a 1.5 s global tap (audio is discarded).
pub fn probe_peak() -> Result<f32> {
    crate::audio::probe_system_audio()
}

/// Refine the System Audio Recording status with a functional probe. Only meaningful while
/// something is playing: silence can mean "denied" or just "nothing playing".
pub fn apply_probe(r: &mut Report, peak: f32) {
    if let Some(c) = r.checks.iter_mut().find(|c| c.id == "system-audio") {
        if peak > 0.0 {
            c.status = Status::Granted;
        } else if c.status != Status::Granted {
            c.status = Status::Unknown;
            c.how = format!(
                "probe heard silence — play any audio (e.g. a YouTube video) and run \
                 `echoctl doctor --probe` again. If it stays silent, {}",
                c.how
            );
        }
    }
    r.ready = r.os_supported && r.checks.iter().filter(|c| c.required).all(|c| c.status == Status::Granted);
}

pub fn report() -> Result<Report> {
    let v = os_version();
    let app = responsible_app();
    let host = app.clone().unwrap_or_else(|| "Echo".into());
    let checks = vec![
        Check {
            id: "microphone",
            name: "Microphone",
            status: mic_status(),
            required: true,
            why: "records your own voice as the 'me' track",
            how: format!(
                "click Allow when macOS asks (tray: Health → Ask for permissions); if previously \
                 denied, enable {host} under Privacy & Security → Microphone"
            ),
            settings_url: Some(MIC_URL),
        },
        Check {
            id: "system-audio",
            name: "System Audio Recording",
            status: system_audio_status(),
            required: true,
            why: "captures what the other participants say, from the Teams app only",
            how: format!(
                "click Allow when macOS asks (tray: Health → Ask for permissions); otherwise enable \
                 {host} under Privacy & Security → Screen & System Audio Recording → System Audio \
                 Recording Only"
            ),
            settings_url: Some(AUDIO_URL),
        },
        Check {
            id: "accessibility",
            name: "Accessibility",
            status: accessibility_status(),
            required: false,
            why: "reads the Teams window to tell who is speaking, so transcripts show real names",
            how: format!(
                "tray: Health → Ask for permissions (opens Settings with {host} listed), then switch \
                 it on under Privacy & Security → Accessibility and restart {host}"
            ),
            settings_url: Some(AX_URL),
        },
        Check {
            id: "screen",
            name: "Screen Recording",
            status: if crate::frames::permitted() { Status::Granted } else { Status::NotDetermined },
            required: false,
            why: "saves snapshots of what is shared on screen during calls (only with the \
                  \"shared screens\" setting on), so the notes include it",
            how: format!(
                "turn on Settings → Save snapshots of shared screens in the menu (macOS asks), or \
                 enable {host} under Privacy & Security → Screen & System Audio Recording (the full \
                 entry, not \"System Audio Recording Only\"), then restart {host}"
            ),
            settings_url: Some(AUDIO_URL),
        },
    ];
    let os_ok = os_supported(&v);
    let ready = os_ok && checks.iter().filter(|c| c.required).all(|c| c.status == Status::Granted);
    Ok(Report {
        platform: "macos",
        os_version: v,
        os_supported: os_ok,
        os_requirement: "macOS 14.2 or later",
        responsible_app: app,
        checks,
        ready,
    })
}

/// Trigger every prompt macOS lets us trigger. Blocks until the dialogs are answered.
pub fn request_all() -> Result<()> {
    if mic_status() == Status::NotDetermined {
        if let Some(media) = unsafe { AVMediaTypeAudio } {
            let (tx, rx) = mpsc::channel();
            let block = RcBlock::new(move |granted: Bool| {
                let _ = tx.send(granted.as_bool());
            });
            unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &block) };
            let _ = rx.recv_timeout(Duration::from_secs(120));
        }
    }
    if system_audio_status() != Status::Granted {
        unsafe {
            let sym = tcc_symbol("TCCAccessRequest");
            if !sym.is_null() {
                let f: RequestFn = std::mem::transmute(sym);
                let (tx, rx) = mpsc::channel();
                let block = RcBlock::new(move |granted: Bool| {
                    let _ = tx.send(granted.as_bool());
                });
                let svc = audio_capture_service();
                f(
                    objc2::rc::Retained::as_ptr(&svc).cast(),
                    std::ptr::null(),
                    RcBlock::as_ptr(&block).cast(),
                );
                let _ = rx.recv_timeout(Duration::from_secs(120));
            }
        }
    }
    if system_audio_status() != Status::Granted {
        eprintln!("starting a short test tap so macOS shows the System Audio Recording prompt…");
        probe_peak()?;
    }
    if crate::config::Config::load().is_ok_and(|c| c.capture_frames) && !crate::frames::permitted() {
        crate::frames::request();
    }
    if accessibility_status() != Status::Granted {
        let key = NSString::from_str("AXTrustedCheckOptionPrompt");
        let opts = NSDictionary::from_slices(&[&*key], &[&*NSNumber::new_bool(true)]);
        unsafe { AXIsProcessTrustedWithOptions(objc2::rc::Retained::as_ptr(&opts).cast()) };
    }
    Ok(())
}
