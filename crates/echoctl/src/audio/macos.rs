//! macOS: capture one app's output with a Core Audio process tap (macOS 14.2+).
//!
//! Flow: find the app's audio process objects -> CATapDescription (mono mixdown of those
//! processes) -> process tap -> private, tap-only aggregate device -> IOProc that forwards
//! input frames to the track writer. The aggregate contains only the tap, so it is clocked by
//! the tap (fixed rate) and keeps working when the user switches output devices. The user must grant "System Audio Recording" to the
//! responsible app (the terminal running Claude Code), otherwise the tap delivers silence.

use std::ffi::{c_void, CStr};
use std::mem::{size_of, MaybeUninit};
use std::sync::atomic::{AtomicU64, Ordering};
use std::ptr::{self, NonNull};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use objc2::rc::{autoreleasepool, Retained};
use objc2::AnyThread;
use objc2_core_audio::*;
use objc2_core_audio_types::{AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp};
use objc2_core_foundation::CFDictionary;
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSObject, NSString};

use super::{downmix, Chunk, Health, LiveStream, Source};
use crate::apps::TargetApp;

fn addr(selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn check(status: i32, what: &str) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(anyhow!("{what} failed (OSStatus {status})"))
    }
}

/// Read a fixed-size property.
unsafe fn get<T: Copy>(obj: AudioObjectID, selector: AudioObjectPropertySelector) -> Result<T> {
    let a = addr(selector);
    let mut size = size_of::<T>() as u32;
    let mut out = MaybeUninit::<T>::uninit();
    check(
        AudioObjectGetPropertyData(
            obj,
            NonNull::from(&a),
            0,
            ptr::null(),
            NonNull::from(&mut size),
            NonNull::new_unchecked(out.as_mut_ptr().cast()),
        ),
        "AudioObjectGetPropertyData",
    )?;
    Ok(out.assume_init())
}

/// Read an array property.
unsafe fn get_vec<T: Copy>(obj: AudioObjectID, selector: AudioObjectPropertySelector) -> Result<Vec<T>> {
    let a = addr(selector);
    let mut size = 0u32;
    check(
        AudioObjectGetPropertyDataSize(obj, NonNull::from(&a), 0, ptr::null(), NonNull::from(&mut size)),
        "AudioObjectGetPropertyDataSize",
    )?;
    let n = size as usize / size_of::<T>();
    let mut v: Vec<T> = Vec::with_capacity(n);
    if n > 0 {
        check(
            AudioObjectGetPropertyData(
                obj,
                NonNull::from(&a),
                0,
                ptr::null(),
                NonNull::from(&mut size),
                NonNull::new_unchecked(v.as_mut_ptr().cast()),
            ),
            "AudioObjectGetPropertyData",
        )?;
        v.set_len(size as usize / size_of::<T>());
    }
    Ok(v)
}

/// Read an array property in a given scope (input/output).
unsafe fn get_vec_scoped<T: Copy>(
    obj: AudioObjectID,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> Result<Vec<T>> {
    let a = AudioObjectPropertyAddress { mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain };
    let mut size = 0u32;
    check(
        AudioObjectGetPropertyDataSize(obj, NonNull::from(&a), 0, ptr::null(), NonNull::from(&mut size)),
        "AudioObjectGetPropertyDataSize",
    )?;
    let n = size as usize / size_of::<T>();
    let mut v: Vec<T> = Vec::with_capacity(n);
    if n > 0 {
        check(
            AudioObjectGetPropertyData(
                obj,
                NonNull::from(&a),
                0,
                ptr::null(),
                NonNull::from(&mut size),
                NonNull::new_unchecked(v.as_mut_ptr().cast()),
            ),
            "AudioObjectGetPropertyData",
        )?;
        v.set_len(size as usize / size_of::<T>());
    }
    Ok(v)
}

/// Read a CFString property (returned +1 retained; NSString is toll-free bridged).
unsafe fn get_string(obj: AudioObjectID, selector: AudioObjectPropertySelector) -> Result<String> {
    let raw: *mut NSString = get(obj, selector)?;
    match Retained::from_raw(raw) {
        // `to_string` autoreleases its UTF-8 copy: drained here, as the health checks poll from worker threads.
        Some(s) => Ok(autoreleasepool(|_| s.to_string())),
        None => Ok(String::new()),
    }
}

#[derive(Debug, Clone)]
pub struct AudioProcess {
    pub object_id: AudioObjectID,
    pub pid: i32,
    pub bundle_id: String,
    pub running_input: bool,
    pub running_output: bool,
}

/// All processes Core Audio currently knows about (processes that have touched audio).
pub fn audio_processes() -> Result<Vec<AudioProcess>> {
    unsafe {
        let ids: Vec<AudioObjectID> =
            get_vec(kAudioObjectSystemObject as AudioObjectID, kAudioHardwarePropertyProcessObjectList)?;
        Ok(ids
            .into_iter()
            .map(|id| AudioProcess {
                object_id: id,
                pid: get::<i32>(id, kAudioProcessPropertyPID).unwrap_or(-1),
                bundle_id: get_string(id, kAudioProcessPropertyBundleID).unwrap_or_default(),
                running_input: get::<u32>(id, kAudioProcessPropertyIsRunningInput).unwrap_or(0) != 0,
                running_output: get::<u32>(id, kAudioProcessPropertyIsRunningOutput).unwrap_or(0) != 0,
            })
            .collect())
    }
}

/// Each process object's pid and bundle id, which don't change while it is listed: asked once, not on every check.
static IDENTITY: std::sync::Mutex<Option<std::collections::HashMap<AudioObjectID, (i32, String)>>> = std::sync::Mutex::new(None);

/// The last answer of `app_processes`, shared by its callers for `FRESH`.
static LAST: std::sync::Mutex<Option<(Instant, TargetApp, Vec<AudioProcess>)>> = std::sync::Mutex::new(None);
/// How old an answer may be. Call detection, the tap and the mic each ask several times a second; each request is
/// work in coreaudiod, on the audio path the call app uses too (measured: ~0.55% of coreaudiod's CPU per check a second).
const FRESH: Duration = Duration::from_millis(750);

/// The app's audio processes: one answer every `FRESH` at most, for all callers. A check lists the process objects
/// (one request) and asks whether they run input or output only of the app's own, not of every process on the Mac.
pub fn app_processes(app: TargetApp) -> Result<Vec<AudioProcess>> {
    shared(&LAST, app, Instant::now(), query_app_processes)
}

/// `last`'s answer for `app` while it is younger than `FRESH`, else a new one from `query`, kept for the next caller.
fn shared<T: Clone>(last: &std::sync::Mutex<Option<(Instant, TargetApp, T)>>, app: TargetApp, now: Instant, query: impl FnOnce(TargetApp) -> Result<T>) -> Result<T> {
    let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, a, v)) = last.as_ref() {
        if *a == app && now.saturating_duration_since(*at) < FRESH {
            return Ok(v.clone());
        }
    }
    let v = query(app)?;
    *last = Some((now, app, v.clone()));
    Ok(v)
}

fn query_app_processes(app: TargetApp) -> Result<Vec<AudioProcess>> {
    let prefixes = app.mac_bundle_prefixes();
    let ids: Vec<AudioObjectID> = unsafe { get_vec(kAudioObjectSystemObject as AudioObjectID, kAudioHardwarePropertyProcessObjectList)? };
    let mut known = IDENTITY.lock().unwrap_or_else(|e| e.into_inner());
    let known = known.get_or_insert_with(Default::default);
    known.retain(|id, _| ids.contains(id));
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            let (pid, bundle_id) = match known.get(&id) {
                Some(k) => k.clone(),
                None => {
                    let read = unsafe { (get::<i32>(id, kAudioProcessPropertyPID).ok(), get_string(id, kAudioProcessPropertyBundleID).ok()) };
                    let k = (read.0.unwrap_or(-1), read.1.clone().unwrap_or_default());
                    // Only what was read: an object asked while it is being created is asked again next time.
                    if let (Some(_), Some(b)) = &read { if !b.is_empty() { known.insert(id, k.clone()); } }
                    k
                }
            };
            prefixes.iter().any(|pre| bundle_id.starts_with(pre)).then(|| AudioProcess {
                object_id: id,
                pid,
                bundle_id,
                running_input: unsafe { get::<u32>(id, kAudioProcessPropertyIsRunningInput) }.unwrap_or(0) != 0,
                running_output: unsafe { get::<u32>(id, kAudioProcessPropertyIsRunningOutput) }.unwrap_or(0) != 0,
            })
        })
        .collect())
}

/// Input devices (UID, name) a process is currently recording from.
fn process_input_devices(proc_obj: AudioObjectID) -> Vec<(String, String)> {
    unsafe {
        get_vec_scoped::<AudioObjectID>(proc_obj, kAudioProcessPropertyDevices, kAudioObjectPropertyScopeInput)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| {
                let uid = get_string(d, kAudioDevicePropertyDeviceUID).ok()?;
                let name = get_string(d, kAudioObjectPropertyName).unwrap_or_default();
                Some((uid, name))
            })
            .collect()
    }
}

/// Names that indicate virtual / loopback / aggregate devices — never the user's real mic.
fn is_virtual_device(uid: &str, name: &str) -> bool {
    let s = format!("{uid} {name}").to_lowercase();
    ["aggregate", "echo", "blackhole", "loopback", "soundflower", "virtual", "zoomaudio", "teams audio", "msteamsaudio"]
        .iter()
        .any(|k| s.contains(k))
}

/// The microphone the app is actually using (Core Audio device UID), if it is recording.
pub fn app_input_device(app: TargetApp) -> Option<(String, String)> {
    app_processes(app)
        .ok()?
        .into_iter()
        .filter(|p| p.running_input)
        .flat_map(|p| process_input_devices(p.object_id))
        // The test call app (debug builds) speaks through a virtual device on purpose: take it as the mic.
        .find(|(uid, name)| !is_virtual_device(uid, name) || crate::apps::test_call_app().is_some())
}

pub fn print_audio_processes() -> Result<()> {
    println!("{:>6}  {:>6}  {:<3} {:<3}  {:<45} mic", "obj", "pid", "in", "out", "bundle id");
    for p in audio_processes()? {
        let mics: Vec<String> = if p.running_input {
            process_input_devices(p.object_id).into_iter().map(|(_, n)| n).collect()
        } else {
            vec![]
        };
        println!(
            "{:>6}  {:>6}  {:<3} {:<3}  {:<45} {}",
            p.object_id,
            p.pid,
            if p.running_input { "yes" } else { "-" },
            if p.running_output { "yes" } else { "-" },
            p.bundle_id,
            mics.join(", ")
        );
    }
    Ok(())
}

fn ns(s: &CStr) -> Retained<NSString> {
    NSString::from_str(s.to_str().expect("utf8 key"))
}

/// State shared with the realtime IOProc.
struct IoCtx {
    tx: Sender<Chunk>,
    epoch: u32,
    rate: u32,
    channels: usize,
    callbacks: AtomicU64,
}

unsafe extern "C-unwind" fn io_proc(
    _device: AudioObjectID,
    _now: NonNull<AudioTimeStamp>,
    input: NonNull<AudioBufferList>,
    _input_time: NonNull<AudioTimeStamp>,
    _output: NonNull<AudioBufferList>,
    _output_time: NonNull<AudioTimeStamp>,
    client: *mut c_void,
) -> i32 {
    let ctx = &*(client as *const IoCtx);
    ctx.callbacks.fetch_add(1, Ordering::Relaxed);
    let list = input.as_ptr();
    let n = (*list).mNumberBuffers as usize;
    let buffers = std::slice::from_raw_parts((*list).mBuffers.as_ptr(), n);
    for b in buffers {
        if b.mData.is_null() {
            continue;
        }
        let samples = std::slice::from_raw_parts(b.mData as *const f32, b.mDataByteSize as usize / 4);
        let ch = if b.mNumberChannels > 0 { b.mNumberChannels as usize } else { ctx.channels.max(1) };
        let _ = ctx.tx.send(Chunk { epoch: ctx.epoch, rate: ctx.rate, samples: downmix(samples, ch) });
    }
    0
}

const STALL_AFTER: Duration = Duration::from_secs(2);
const PROCESS_CHECK_EVERY: Duration = Duration::from_secs(1);

/// The target app's audio. Reopened whenever the app's set of audio processes changes
/// (Teams spawns helper processes as calls start, screens are shared, etc.).
pub struct AppTapSource {
    app: TargetApp,
}

impl AppTapSource {
    pub fn new(app: TargetApp) -> Self {
        Self { app }
    }
}

impl Source for AppTapSource {
    fn open(&mut self, epoch: u32, tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
        let procs = app_processes(self.app)?;
        if procs.is_empty() {
            bail!("{:?} has no audio processes yet — waiting for the call to start", self.app);
        }
        let ids: Vec<AudioObjectID> = procs.iter().map(|p| p.object_id).collect();
        let nums: Vec<Retained<NSNumber>> = ids.iter().map(|id| NSNumber::new_u32(*id)).collect();
        let list = NSArray::from_retained_slice(&nums);
        let desc = unsafe { CATapDescription::initMonoMixdownOfProcesses(CATapDescription::alloc(), &list) };
        let mut tap = TapStream::start(desc, epoch, tx)?;
        tap.app = Some(self.app);
        tap.label = format!(
            "{:?} tap on {} @ {} Hz",
            self.app,
            procs.iter().map(|p| format!("{}[{}]", p.bundle_id, p.pid)).collect::<Vec<_>>().join(", "),
            tap.rate
        );
        tap.tapped = ids;
        Ok(Box::new(tap))
    }
}

/// Run a short global tap and return the peak level it saw. The first tap a process starts
/// is what makes macOS show the "System Audio Recording" prompt, and a denied tap delivers
/// pure silence — so this doubles as the only reliable permission check. Audio is discarded.
pub fn probe_system_audio() -> Result<f32> {
    let none: Retained<NSArray<NSNumber>> = NSArray::new();
    let desc = unsafe { CATapDescription::initMonoGlobalTapButExcludeProcesses(CATapDescription::alloc(), &none) };
    let (tx, rx) = mpsc::channel();
    let tap = TapStream::start(desc, 1, tx)?;
    std::thread::sleep(Duration::from_millis(1500));
    drop(tap);
    let (mut frames, mut peak) = (0usize, 0f32);
    for c in rx.try_iter() {
        frames += c.samples.len();
        peak = c.samples.iter().fold(peak, |m, s| m.max(s.abs()));
    }
    if std::env::var_os("ECHOCTL_DEBUG").is_some() {
        eprintln!("debug: probe frames={frames} peak={peak}");
    }
    Ok(peak)
}

pub struct TapStream {
    tap_id: AudioObjectID,
    agg_id: AudioObjectID,
    proc_id: AudioDeviceIOProcID,
    ctx: *mut IoCtx,
    rate: u32,
    label: String,
    app: Option<TargetApp>,
    tapped: Vec<AudioObjectID>,
    seen: (u64, Instant),
    last_process_check: Instant,
}

impl TapStream {
    fn start(desc: Retained<CATapDescription>, epoch: u32, tx: Sender<Chunk>) -> Result<Self> {
        unsafe {
            // 1. Private tap; the tapped audio keeps playing for the user.
            desc.setName(&NSString::from_str("echo"));
            desc.setPrivate(true);
            desc.setMuteBehavior(CATapMuteBehavior::Unmuted);
            let mut tap_id: AudioObjectID = 0;
            check(AudioHardwareCreateProcessTap(Some(&desc), &mut tap_id), "AudioHardwareCreateProcessTap")?;
            let tap_uid = desc.UUID().UUIDString();

            // Owns everything created so far; Drop tears down whatever exists on early return.
            let mut me = TapStream {
                tap_id,
                agg_id: 0,
                proc_id: None,
                ctx: ptr::null_mut(),
                rate: 0,
                label: "tap".into(),
                app: None,
                tapped: vec![],
                seen: (0, Instant::now()),
                last_process_check: Instant::now(),
            };

            // 2. Tap-only private aggregate device: clocked by the tap, independent of outputs.
            let sub_tap = NSDictionary::<NSString, NSObject>::from_slices(
                &[&*ns(kAudioSubTapUIDKey), &*ns(kAudioSubTapDriftCompensationKey)],
                &[&**tap_uid as &NSObject, &**NSNumber::new_bool(true) as &NSObject],
            );
            let taps = NSArray::from_retained_slice(&[sub_tap]);
            let agg_uid = NSString::from_str(&format!("echo-aggregate-{}-{tap_id}", std::process::id()));
            let agg_name = NSString::from_str("echo");
            let (yes, no) = (NSNumber::new_bool(true), NSNumber::new_bool(false));
            let agg_desc = NSDictionary::<NSString, NSObject>::from_slices(
                &[
                    &*ns(kAudioAggregateDeviceNameKey),
                    &*ns(kAudioAggregateDeviceUIDKey),
                    &*ns(kAudioAggregateDeviceIsPrivateKey),
                    &*ns(kAudioAggregateDeviceIsStackedKey),
                    &*ns(kAudioAggregateDeviceTapAutoStartKey),
                    &*ns(kAudioAggregateDeviceTapListKey),
                ],
                &[
                    &**agg_name as &NSObject,
                    &**agg_uid as &NSObject,
                    &**yes as &NSObject,
                    &**no as &NSObject,
                    &**yes as &NSObject,
                    &**taps as &NSObject,
                ],
            );
            let cf: &CFDictionary = &*(Retained::as_ptr(&agg_desc) as *const CFDictionary);
            check(
                AudioHardwareCreateAggregateDevice(cf, NonNull::from(&mut me.agg_id)),
                "AudioHardwareCreateAggregateDevice",
            )?;

            // 3. Format: channels from the tap, rate from the device that actually clocks us.
            let fmt: AudioStreamBasicDescription = get(tap_id, kAudioTapPropertyFormat)?;
            let rate = get::<f64>(me.agg_id, kAudioDevicePropertyNominalSampleRate).unwrap_or(fmt.mSampleRate);
            if std::env::var_os("ECHOCTL_DEBUG").is_some() {
                eprintln!("debug: tap format {} Hz x{} ch, aggregate {rate} Hz", fmt.mSampleRate, fmt.mChannelsPerFrame);
            }
            if !(8_000.0..=384_000.0).contains(&rate) {
                bail!("the tap reports an unusable sample rate ({rate} Hz)");
            }
            me.rate = rate as u32;
            me.ctx = Box::into_raw(Box::new(IoCtx {
                tx,
                epoch,
                rate: me.rate,
                channels: fmt.mChannelsPerFrame as usize,
                callbacks: AtomicU64::new(0),
            }));

            // 4. IOProc + start.
            check(
                AudioDeviceCreateIOProcID(me.agg_id, Some(io_proc), me.ctx.cast(), NonNull::from(&mut me.proc_id)),
                "AudioDeviceCreateIOProcID",
            )?;
            check(AudioDeviceStart(me.agg_id, me.proc_id), "AudioDeviceStart")?;
            Ok(me)
        }
    }
}

impl LiveStream for TapStream {
    fn label(&self) -> String {
        self.label.clone()
    }

    fn health(&mut self) -> Health {
        // A tap-only aggregate is clocked by the tapped processes: it delivers frames only while
        // they output audio. No callbacks while the app *is* outputting = broken; while it's
        // idle it's just silence, which the track writer pads from the wall clock.
        let n = unsafe { (*self.ctx).callbacks.load(Ordering::Relaxed) };
        if n != self.seen.0 {
            self.seen = (n, Instant::now());
        } else if self.seen.1.elapsed() > STALL_AFTER {
            let app_outputting = self
                .app
                .and_then(|a| app_processes(a).ok())
                .is_none_or(|ps| ps.iter().any(|p| p.running_output));
            if app_outputting {
                return Health::Restart("tap stopped delivering audio".into());
            }
            self.seen.1 = Instant::now(); // idle app: check again later
        }
        if let Ok(r) = unsafe { get::<f64>(self.agg_id, kAudioDevicePropertyNominalSampleRate) } {
            if r as u32 != self.rate {
                return Health::Reopen(format!("rate changed {} → {} Hz", self.rate, r as u32));
            }
        }
        if let Some(app) = self.app {
            if self.last_process_check.elapsed() >= PROCESS_CHECK_EVERY {
                self.last_process_check = Instant::now();
                if let Ok(procs) = app_processes(app) {
                    let mut now: Vec<AudioObjectID> = procs.iter().map(|p| p.object_id).collect();
                    let mut was = self.tapped.clone();
                    now.sort_unstable();
                    was.sort_unstable();
                    if !now.is_empty() && now != was {
                        return Health::Reopen(format!(
                            "{app:?} audio processes changed ({} → {})",
                            was.len(),
                            now.len()
                        ));
                    }
                }
            }
        }
        Health::Ok
    }
}

impl Drop for TapStream {
    fn drop(&mut self) {
        unsafe {
            if self.proc_id.is_some() {
                AudioDeviceStop(self.agg_id, self.proc_id);
                AudioDeviceDestroyIOProcID(self.agg_id, self.proc_id);
            }
            if self.agg_id != 0 {
                AudioHardwareDestroyAggregateDevice(self.agg_id);
            }
            if self.tap_id != 0 {
                AudioHardwareDestroyProcessTap(self.tap_id);
            }
            if !self.ctx.is_null() {
                drop(Box::from_raw(self.ctx));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn callers_share_one_answer_about_the_app_for_a_moment() {
        let last = std::sync::Mutex::new(None);
        let asked = Cell::new(0);
        let query = |_: TargetApp| { asked.set(asked.get() + 1); Ok(asked.get()) };
        let t0 = Instant::now();
        assert_eq!(shared(&last, TargetApp::Teams, t0, query).unwrap(), 1);
        // Call detection, the tap and the mic asking within the moment: coreaudiod is asked once.
        for ms in [100, 400, 700] {
            assert_eq!(shared(&last, TargetApp::Teams, t0 + Duration::from_millis(ms), query).unwrap(), 1);
        }
        assert_eq!(shared(&last, TargetApp::Teams, t0 + FRESH, query).unwrap(), 2, "asked again once it is older");
        assert_eq!(asked.get(), 2);
    }
}
