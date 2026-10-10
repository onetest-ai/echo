//! Windows: capture one app's output with WASAPI process loopback (Windows 10 2004+).
//!
//! Teams (`ms-teams.exe`) renders audio from msedgewebview2 child processes, so we activate the
//! loopback in INCLUDE_TARGET_PROCESS_TREE mode on the root Teams process. No permission is
//! needed for loopback capture on Windows. Loopback follows the process, not a device, so
//! output-device switches are handled by Windows; we request a fixed format and Windows
//! converts. Loopback delivers no packets while the app is silent — the track writer pads
//! those gaps from the wall clock, so silence is not treated as a stall.

use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use windows::core::{implement, Interface, Ref, GUID, HRESULT};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
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

/// Root processes of the app: matching exe whose parent is not also the app.
pub fn app_roots(app: TargetApp) -> Result<Vec<Proc>> {
    let all = processes()?;
    let names = app.windows_exe_names();
    let is_app = |p: &Proc| names.contains(&p.exe.as_str());
    Ok(all
        .iter()
        .filter(|p| is_app(p) && !all.iter().any(|q| q.pid == p.parent && is_app(q)))
        .cloned()
        .collect())
}

pub fn print_audio_processes() -> Result<()> {
    // TODO(detect): enumerate WASAPI sessions (IAudioSessionManager2) to show who uses the mic.
    for p in app_roots(TargetApp::Teams)? {
        println!("{:>6}  {}", p.pid, p.exe);
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

fn capture_loop(pid: u32, epoch: u32, tx: Sender<Chunk>, stop: Arc<AtomicBool>, ready: Sender<Result<()>>) {
    let run = || -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let client = activate_loopback(pid)?;
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
}

impl LoopbackSource {
    pub fn new(app: TargetApp) -> Self {
        Self { app }
    }
}

struct LoopbackStream {
    app: TargetApp,
    pid: u32,
    label: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    last_process_check: Instant,
}

impl Source for LoopbackSource {
    fn open(&mut self, epoch: u32, tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
        let roots = app_roots(self.app)?;
        let Some(root) = roots.first().cloned() else {
            bail!("{:?} is not running — waiting for it to start", self.app);
        };
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let s = stop.clone();
        let thread = std::thread::spawn(move || capture_loop(root.pid, epoch, tx, s, ready_tx));
        ready_rx.recv().map_err(|_| anyhow!("loopback thread exited"))??;
        Ok(Box::new(LoopbackStream {
            app: self.app,
            pid: root.pid,
            label: format!("{:?} loopback on {}[{}] + children @ {SAMPLE_RATE} Hz", self.app, root.exe, root.pid),
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
        if self.last_process_check.elapsed() >= PROCESS_CHECK_EVERY {
            self.last_process_check = Instant::now();
            if let Ok(roots) = app_roots(self.app) {
                if !roots.iter().any(|r| r.pid == self.pid) {
                    return Health::Restart(format!("{:?} process {} went away", self.app, self.pid));
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
