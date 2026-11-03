//! Windows: capture the call app's audio with WASAPI loopback (Windows 10 2004+).
//!
//! New Teams is "process-loopback blind": per-process loopback (INCLUDE_TARGET_PROCESS_TREE on
//! `ms-teams.exe`) delivers only digital silence for its call audio, although its media process
//! (an `ms-teams.exe` child) has an active playback session — measured with Teams 26225 on
//! Windows 11, while the same code records any other process. So the remote track is a
//! *device* loopback of the output device Teams plays on, found from its playback sessions and
//! followed when Teams switches device (headset ↔ speakers); with no session, the last device
//! or the default output — only while Teams is running. Other sounds on that device (other
//! apps, notifications) are recorded too: unlike macOS, Windows can't isolate Teams' audio.
//! `ECHOCTL_LOOPBACK=process` restores process loopback (`ECHOCTL_LOOPBACK_PID=<pid>` targets
//! any process tree, for diagnostics). No permission is needed for loopback capture. We
//! request a fixed format and Windows converts. Loopback delivers no packets while nothing
//! plays — the track writer pads those gaps from the wall clock, so silence is not a stall.
//!
//! Call presence: Teams holds the mic during a call, which shows up as an active WASAPI
//! capture session owned by a process in its tree (the same signal as macOS's "process is
//! running input"); that session's endpoint is the mic the call is using.

use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use windows::core::{implement, Interface, Ref, GUID, HRESULT};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject, INFINITE};

use super::{downmix, Chunk, Health, LiveStream, Source};
use crate::apps::TargetApp;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub parent: u32,
    pub exe: String,
}

pub fn processes() -> Result<Vec<Proc>> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut out = vec![];
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                out.push(Proc {
                    pid: entry.th32ProcessID,
                    parent: entry.th32ParentProcessID,
                    exe: String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase(),
                });
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        Ok(out)
    }
}

/// Root processes of the app in a snapshot: matching exe whose parent is not also the app.
fn roots_in(all: &[Proc], app: TargetApp) -> Vec<Proc> {
    let names = app.windows_exe_names();
    let is_app = |p: &Proc| names.contains(&p.exe.as_str());
    all.iter().filter(|p| is_app(p) && !all.iter().any(|q| q.pid == p.parent && is_app(q))).cloned().collect()
}

/// Root processes of the app.
pub fn app_roots(app: TargetApp) -> Result<Vec<Proc>> {
    Ok(roots_in(&processes()?, app))
}

/// A result shared by every caller within `ttl`: the watcher, the mic track and the remote
/// track each ask about Teams about once a second, and a process snapshot or an audio-session
/// enumeration costs milliseconds each.
fn shared<T: Clone>(slot: &Mutex<Option<(Instant, String, T)>>, key: String, ttl: Duration, get: impl FnOnce() -> Result<T>) -> Result<T> {
    let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, k, v)) = s.as_ref() {
        if *k == key && at.elapsed() < ttl {
            return Ok(v.clone());
        }
    }
    let v = get()?;
    *s = Some((Instant::now(), key, v.clone()));
    Ok(v)
}

/// Pids of the app's processes and all their descendants (Teams records from helpers), from
/// one snapshot; shared for 2 s (helpers come and go rarely; call detection debounces anyway).
pub fn app_tree(app: TargetApp) -> Result<Vec<u32>> {
    static TREE: Mutex<Option<(Instant, String, Vec<u32>)>> = Mutex::new(None);
    shared(&TREE, format!("{app:?}"), Duration::from_secs(2), || {
        let all = processes()?;
        let mut tree: Vec<u32> = roots_in(&all, app).iter().map(|p| p.pid).collect();
        let mut i = 0;
        while i < tree.len() {
            let parent = tree[i];
            let children: Vec<u32> =
                all.iter().filter(|p| p.parent == parent && !tree.contains(&p.pid)).map(|p| p.pid).collect();
            tree.extend(children);
            i += 1;
        }
        Ok(tree)
    })
}

/// One WASAPI capture session: who owns it, on which endpoint, and whether it is running.
#[derive(Debug, Clone)]
pub struct CaptureSession {
    pub pid: u32,
    pub endpoint: String,
    pub active: bool,
}

/// COM for the current thread for the length of a call; tolerates a thread already in an STA.
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

/// Capture sessions on every active input endpoint.
pub fn capture_sessions() -> Result<Vec<CaptureSession>> {
    sessions(eCapture)
}

/// Audio sessions on every active endpoint of one direction (eCapture: mics, eRender: output).
fn sessions(flow: EDataFlow) -> Result<Vec<CaptureSession>> {
    let _com = Com::init();
    let mut out = vec![];
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let endpoints = devices.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
        for i in 0..endpoints.GetCount()? {
            let Ok(dev) = endpoints.Item(i) else { continue };
            let Ok(id) = dev.GetId() else { continue };
            let endpoint = id.to_string().unwrap_or_default();
            windows::Win32::System::Com::CoTaskMemFree(Some(id.0 as *const _));
            let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(sessions) = mgr.GetSessionEnumerator() else { continue };
            for j in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(ctl) = sessions.GetSession(j) else { continue };
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
                let active = ctl.GetState().is_ok_and(|s| s == AudioSessionStateActive);
                out.push(CaptureSession { pid: ctl2.GetProcessId().unwrap_or(0), endpoint: endpoint.clone(), active });
            }
        }
    }
    Ok(out)
}

/// The app's running capture sessions (non-empty ⇒ it holds a mic ⇒ in a call); shared
/// within a tick by call detection and the mic track.
fn app_capture_sessions(app: TargetApp) -> Result<Vec<CaptureSession>> {
    static SESSIONS: Mutex<Option<(Instant, String, Vec<CaptureSession>)>> = Mutex::new(None);
    shared(&SESSIONS, format!("{app:?}"), Duration::from_millis(900), || {
        let tree = app_tree(app)?;
        Ok(capture_sessions()?.into_iter().filter(|s| s.active && tree.contains(&s.pid)).collect())
    })
}

pub fn app_in_call(app: TargetApp) -> bool {
    app_capture_sessions(app).is_ok_and(|s| !s.is_empty())
}

/// Friendly name of an input endpoint (via cpal, whose WASAPI device id is the endpoint id).
fn endpoint_name(endpoint: &str) -> Option<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host()
        .input_devices()
        .ok()?
        .find(|d| d.id().is_ok_and(|id| id.id() == endpoint))
        .and_then(|d| d.description().ok().map(|d| d.to_string()))
}

/// The mic the app is recording from, as (endpoint id, name).
pub fn app_input_device(app: TargetApp) -> Option<(String, String)> {
    let s = app_capture_sessions(app).ok()?.into_iter().next()?;
    let name = endpoint_name(&s.endpoint).unwrap_or_else(|| s.endpoint.clone());
    Some((s.endpoint, name))
}

pub fn print_audio_processes() -> Result<()> {
    let tree = app_tree(TargetApp::Teams)?;
    let names: std::collections::HashMap<u32, String> = processes()?.into_iter().map(|p| (p.pid, p.exe)).collect();
    println!("Teams processes: {}", if tree.is_empty() { "none".into() } else { format!("{} (root {})", tree.len(), tree[0]) });
    println!("{:>6}  {:<6}  {:<5}  {:<28} mic", "pid", "state", "teams", "process");
    for s in capture_sessions()? {
        let exe = names.get(&s.pid).cloned().unwrap_or_else(|| if s.pid == 0 { "(system)".into() } else { "?".into() });
        let mic = endpoint_name(&s.endpoint).unwrap_or_else(|| s.endpoint.clone());
        let state = if s.active { "active" } else { "idle" };
        let teams = if tree.contains(&s.pid) { "yes" } else { "" };
        println!("{:>6}  {state:<6}  {teams:<5}  {exe:<28} {mic}", s.pid);
    }
    println!("in call: {}", app_in_call(TargetApp::Teams));
    // Who is playing sound: the remote track records the Teams process tree's output.
    println!("\nplayback sessions:");
    for s in sessions(eRender)?.into_iter().filter(|s| s.active) {
        let exe = names.get(&s.pid).cloned().unwrap_or_else(|| "?".into());
        let teams = if tree.contains(&s.pid) { "yes" } else { "" };
        println!("{:>6}  {:<6}  {teams:<5}  {exe:<28} {}", s.pid, "active", s.endpoint);
    }
    Ok(())
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Activated {
    event: HANDLE,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for Activated_Impl {
    fn ActivateCompleted(&self, _op: Ref<IActivateAudioInterfaceAsyncOperation>) -> windows::core::Result<()> {
        unsafe { SetEvent(self.event) }
    }
}

/// PROPVARIANT holding a VT_BLOB, laid out by hand (x64 layout of tagPROPVARIANT).
#[repr(C)]
struct BlobPropVariant {
    vt: u16,
    reserved: [u16; 3],
    cb_size: u32,
    p_blob_data: *const u8,
    _pad: usize,
}
const VT_BLOB: u16 = 65;

unsafe fn activate_loopback(pid: u32) -> Result<IAudioClient> {
    let params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    let blob = BlobPropVariant {
        vt: VT_BLOB,
        reserved: [0; 3],
        cb_size: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
        p_blob_data: &params as *const _ as *const u8,
        _pad: 0,
    };
    let event = CreateEventW(None, false, false, None)?;
    let handler: IActivateAudioInterfaceCompletionHandler = Activated { event }.into();
    let op = ActivateAudioInterfaceAsync(
        VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
        &IAudioClient::IID as *const GUID,
        Some(&blob as *const _ as *const PROPVARIANT),
        &handler,
    )?;
    WaitForSingleObject(event, INFINITE);
    let _ = CloseHandle(event);
    let mut hr = HRESULT(0);
    let mut unk = None;
    op.GetActivateResult(&mut hr, &mut unk)?;
    hr.ok()?;
    Ok(unk.ok_or_else(|| anyhow!("loopback activation returned no interface"))?.cast()?)
}

/// What the remote track loops back.
#[derive(Clone, PartialEq)]
enum Target {
    /// A process tree (process loopback, root pid).
    Process(u32),
    /// An output device (endpoint id).
    Device(String),
}

unsafe fn activate_device(endpoint: &str) -> Result<IAudioClient> {
    let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let dev = devices.GetDevice(&windows::core::HSTRING::from(endpoint))?;
    Ok(dev.Activate::<IAudioClient>(CLSCTX_ALL, None)?)
}

/// The output device the app plays on: an endpoint with a playback session of its process
/// tree — an active one (`current` first: Teams can play on two devices at once, the call on
/// one and its tones on another), or with `idle_too` any (Teams keeps its session on the
/// speaker chosen in Teams between sounds).
fn app_render_endpoint(app: TargetApp, current: Option<&str>, idle_too: bool) -> Option<String> {
    let tree = app_tree(app).ok()?;
    let ours: Vec<CaptureSession> = sessions(eRender).ok()?.into_iter().filter(|s| tree.contains(&s.pid)).collect();
    let active = || ours.iter().filter(|s| s.active);
    active()
        .find(|s| Some(s.endpoint.as_str()) == current)
        .or_else(|| active().next())
        .or_else(|| ours.iter().find(|_| idle_too))
        .map(|s| s.endpoint.clone())
}

/// The default output device.
fn default_output() -> Option<String> {
    let _com = Com::init();
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let dev = devices.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let id = dev.GetId().ok()?;
        let s = id.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(id.0 as *const _));
        s
    }
}

/// Friendly name of an output device.
fn output_name(endpoint: &str) -> String {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host()
        .output_devices()
        .ok()
        .and_then(|mut ds| ds.find(|d| d.id().is_ok_and(|id| id.id() == endpoint)))
        .and_then(|d| d.description().ok().map(|d| d.to_string()))
        .unwrap_or_else(|| endpoint.to_string())
}

fn capture_loop(target: Target, epoch: u32, tx: Sender<Chunk>, stop: Arc<AtomicBool>, ready: Sender<Result<()>>) {
    let run = || -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let client = match &target {
                Target::Process(pid) => activate_loopback(*pid)?,
                Target::Device(endpoint) => activate_device(endpoint)?,
            };
            let block = CHANNELS * 4;
            let fmt = WAVEFORMATEX {
                wFormatTag: 3, // WAVE_FORMAT_IEEE_FLOAT
                nChannels: CHANNELS,
                nSamplesPerSec: SAMPLE_RATE,
                nAvgBytesPerSec: SAMPLE_RATE * block as u32,
                nBlockAlign: block,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                200_000, // 20 ms, in 100 ns units
                0,
                &fmt,
                None,
            )?;
            let event = CreateEventW(None, false, false, None)?;
            client.SetEventHandle(event)?;
            let cap: IAudioCaptureClient = client.GetService()?;
            client.Start()?;
            let _ = ready.send(Ok(()));

            while !stop.load(Ordering::SeqCst) {
                if WaitForSingleObject(event, 100) != WAIT_OBJECT_0 {
                    continue;
                }
                while cap.GetNextPacketSize()? > 0 {
                    let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                    cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                    let n = frames as usize * CHANNELS as usize;
                    let mono = if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || data.is_null() {
                        vec![0.0; frames as usize]
                    } else {
                        downmix(std::slice::from_raw_parts(data as *const f32, n), CHANNELS as usize)
                    };
                    cap.ReleaseBuffer(frames)?;
                    let _ = tx.send(Chunk { epoch, rate: SAMPLE_RATE, samples: mono });
                }
            }
            client.Stop()?;
            let _ = CloseHandle(event);
            CoUninitialize();
            Ok(())
        }
    };
    if let Err(e) = run() {
        let _ = ready.send(Err(e));
    }
}

const PROCESS_CHECK_EVERY: Duration = Duration::from_secs(1);

pub struct LoopbackSource {
    app: TargetApp,
    /// The device captured last: kept while the app plays on no device (between sounds).
    device: Option<String>,
}

impl LoopbackSource {
    pub fn new(app: TargetApp) -> Self {
        Self { app, device: None }
    }
}

struct LoopbackStream {
    app: TargetApp,
    target: Target,
    label: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    last_process_check: Instant,
}

impl Source for LoopbackSource {
    fn open(&mut self, epoch: u32, tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
        let (target, label) = if let Some(pid) = std::env::var("ECHOCTL_LOOPBACK_PID").ok().and_then(|p| p.parse().ok()) {
            (Target::Process(pid), format!("process loopback on pid {pid} + children @ {SAMPLE_RATE} Hz"))
        } else if std::env::var("ECHOCTL_LOOPBACK").is_ok_and(|m| m == "process") {
            let Some(root) = app_roots(self.app)?.first().cloned() else {
                bail!("{:?} is not running — waiting for it to start", self.app);
            };
            let label = format!("{:?} process loopback on {}[{}] + children @ {SAMPLE_RATE} Hz", self.app, root.exe, root.pid);
            (Target::Process(root.pid), label)
        } else {
            // Never record an output device for an app that isn't there.
            if app_roots(self.app)?.is_empty() {
                bail!("{:?} is not running — waiting for it to start", self.app);
            }
            let (endpoint, how) = match app_render_endpoint(self.app, self.device.as_deref(), true) {
                Some(e) => (e, format!("the device {:?} plays on", self.app)),
                None => match self.device.clone().or_else(default_output) {
                    Some(e) => (e, "the default output".to_string()),
                    None => bail!("no output device to record from"),
                },
            };
            self.device = Some(endpoint.clone());
            let label = format!("output loopback on \"{}\" ({how}) @ {SAMPLE_RATE} Hz", output_name(&endpoint));
            (Target::Device(endpoint), label)
        };
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let s = stop.clone();
        let t = target.clone();
        let thread = std::thread::spawn(move || capture_loop(t, epoch, tx, s, ready_tx));
        ready_rx.recv().map_err(|_| anyhow!("loopback thread exited"))??;
        Ok(Box::new(LoopbackStream {
            app: self.app,
            target,
            label,
            stop,
            thread: Some(thread),
            last_process_check: Instant::now(),
        }))
    }
}

impl LiveStream for LoopbackStream {
    fn label(&self) -> String {
        self.label.clone()
    }

    fn health(&mut self) -> Health {
        if self.thread.as_ref().is_some_and(|t| t.is_finished()) {
            return Health::Restart("loopback capture thread exited".into());
        }
        if self.last_process_check.elapsed() < PROCESS_CHECK_EVERY || std::env::var_os("ECHOCTL_LOOPBACK_PID").is_some() {
            return Health::Ok;
        }
        self.last_process_check = Instant::now();
        match &self.target {
            Target::Process(pid) => {
                if let Ok(roots) = app_roots(self.app) {
                    if !roots.iter().any(|r| r.pid == *pid) {
                        return Health::Restart(format!("{:?} process {pid} went away", self.app));
                    }
                }
            }
            // Follow the app to another output device — only when it is playing there, so a
            // quiet moment (no active session anywhere) keeps the current device.
            Target::Device(current) => {
                if app_tree(self.app).is_ok_and(|t| t.is_empty()) {
                    return Health::Restart(format!("{:?} quit", self.app));
                }
                if let Some(e) = app_render_endpoint(self.app, Some(current), false).filter(|e| e != current) {
                    return Health::Reopen(format!("{:?} switched output to \"{}\"", self.app, output_name(&e)));
                }
            }
        }
        Health::Ok
    }
}

impl Drop for LoopbackStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
