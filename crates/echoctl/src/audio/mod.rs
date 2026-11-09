//! Audio capture.
//!
//! A *Source* knows how to open a live capture (a mic stream, a process tap). A *Track*
//! supervises one source on a thread of its own: it polls the live stream's health and rebuilds it whenever the
//! device, sample rate or app processes change, so the user can switch headphones or mics
//! mid-call; a device that hangs holds up only that thread. Every incarnation streams mono `Chunk`s to the track's writer thread, which
//! resamples to a fixed rate and pads gaps with silence against a shared wall clock — so all
//! tracks of a call stay aligned no matter how often a source is rebuilt.

pub mod mic;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(debug_assertions)]
mod testfeed;
#[cfg(windows)]
pub(crate) mod windows;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::apps::TargetApp;

/// Rate tracks are stored at: whisper's native rate. 16-bit PCM ≈ 115 MB per track-hour.
pub const TRACK_RATE: u32 = 16_000;
/// Gaps shorter than this are treated as scheduling jitter, not missing audio.
const GAP_TOLERANCE: Duration = Duration::from_millis(200);

/// Mono samples from one incarnation (`epoch`) of a source.
pub struct Chunk {
    pub epoch: u32,
    pub rate: u32,
    pub samples: Vec<f32>,
}

pub enum Health {
    Ok,
    /// Something failed (an error, a stall): tear down and reopen, backing off when it repeats.
    Restart(String),
    /// A planned change (another device, a new rate, the app's processes): reopen at once, without
    /// counting toward the failure backoff.
    Reopen(String),
}

/// One running capture. Dropping it stops the capture.
pub trait LiveStream {
    /// Human-readable description of what is being captured (device / processes, rate).
    fn label(&self) -> String;
    /// Called every poll (~250 ms).
    fn health(&mut self) -> Health;
}

pub trait Source: Send {
    fn open(&mut self, epoch: u32, tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>>;
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackEvent {
    /// Seconds since the call started
    pub at: f64,
    pub kind: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackInfo {
    pub file: String,
    pub sample_rate: u32,
    pub frames: u64,
    pub padded_frames: u64,
    pub events: Vec<TrackEvent>,
}

const RETRY_EVERY: Duration = Duration::from_secs(1);
/// Restart policy for failures (`Health::Restart`; a planned `Reopen` reopens at once): back off 1 s → 30 s between restarts;
/// beyond 20 restarts only retry every 30 s; a stream stable for 5 min resets the count.
const BACKOFF_MAX: Duration = Duration::from_secs(30);
const RESTARTS_BEFORE_SLOW: u32 = 20;
const STABLE_RESETS: Duration = Duration::from_secs(5 * 60);
/// Audio arriving as exact digital zeros this long means a dead stream (or a revoked
/// permission, which macOS signals with zeros): reopen it, at most this many times.
const SILENT_RESTART_AFTER: Duration = Duration::from_secs(5 * 60);
const SILENT_RESTARTS_MAX: u32 = 10;

/// What the writer saw: ms since t0 of the last chunk, and of the last non-zero chunk; and why
/// it stopped writing, when it failed (a full disk, a volume gone).
#[derive(Default)]
struct Signal {
    last_chunk_ms: AtomicU64,
    last_sound_ms: AtomicU64,
    failed: Mutex<Option<String>>,
    /// The track ended without its device (it hung): the writer stops now, not when the device lets go.
    halted: AtomicBool,
    /// The track was told to stop: a supervisor back from a hang opens nothing more.
    stopping: AtomicBool,
}

/// How often a track's supervisor looks at its stream.
const SUPERVISE_EVERY: Duration = Duration::from_millis(250);
/// How long a track's end waits for its device to let go: one that hangs never holds up the stop.
const LET_GO_WITHIN: Duration = Duration::from_secs(1);

/// One track of a recording: its writer, and its source supervised on a thread of its own (`Supervisor`), so the
/// caller (the watcher's loop) never waits on a device.
pub struct Track {
    name: &'static str,
    file: String,
    tx: Option<Sender<Chunk>>,
    writer: Option<WriterHandle>,
    signal: Arc<Signal>,
    events: Arc<Mutex<Vec<TrackEvent>>>,
    t0: Instant,
    /// Dropped to halt the supervisor.
    halt: Option<Sender<()>>,
    /// When it was halted: every track of a recording waits for its device from the same moment.
    halted_at: Option<Instant>,
    /// The writer's error, sent once by the supervisor.
    failed: Receiver<String>,
    /// Sent when the supervisor has let go of the device.
    done: Receiver<()>,
}

/// What supervises one source: opens it, watches its health, rebuilds it.
struct Supervisor {
    name: &'static str,
    source: Box<dyn Source>,
    live: Option<Box<dyn LiveStream>>,
    epoch: u32,
    tx: Sender<Chunk>,
    t0: Instant,
    next_retry: Instant,
    last_error: Option<String>,
    events: Arc<Mutex<Vec<TrackEvent>>>,
    restarts: u32,
    silent_restarts: u32,
    opened_at: Instant,
    signal: Arc<Signal>,
    /// Reopen when the stream delivers only zeros for too long (the call app's track).
    watch_silence: bool,
    /// The writer's failure has been reported (`write_failure`).
    write_failed: bool,
    /// When the last planned reopens happened: more than `REOPENS_PER_WINDOW` within
    /// `REOPEN_WINDOW` (a device or a process set flipping back and forth) back off like failures.
    reopens: Vec<Instant>,
}

/// Planned reopens are free up to this many within `REOPEN_WINDOW`; beyond, they back off.
const REOPENS_PER_WINDOW: usize = 3;
const REOPEN_WINDOW: Duration = Duration::from_secs(30);

impl Track {
    /// Start a track. If the source can't open yet (e.g. Teams isn't playing audio), the track
    /// keeps retrying every second instead of failing.
    pub fn start(name: &'static str, path: &Path, source: Box<dyn Source>, t0: Instant) -> Result<Self> {
        let signal = Arc::new(Signal::default());
        let (tx, writer) = spawn_writer(path, t0, signal.clone())?;
        let events = Arc::new(Mutex::new(vec![]));
        let (halt, halted) = mpsc::channel::<()>();
        let (failed_tx, failed) = mpsc::channel();
        let (done_tx, done) = mpsc::channel();
        let (sup_tx, sup_signal, sup_events) = (tx.clone(), signal.clone(), events.clone());
        // Made on its thread: the streams it opens stay there (a device's stream isn't always `Send`).
        std::thread::Builder::new().name(format!("track-{name}")).spawn(move || {
            let mut sup = Supervisor::new(name, source, sup_tx, t0, sup_signal, sup_events);
            sup.try_open();
            // Until halted (the sender dropped): no message is ever sent.
            while let Err(RecvTimeoutError::Timeout) = halted.recv_timeout(SUPERVISE_EVERY) {
                if let Some(e) = sup.poll() {
                    let _ = failed_tx.send(e);
                }
            }
            sup.live = None;
            drop(sup);
            let _ = done_tx.send(());
        })?;
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        Ok(Track { name, file, tx: Some(tx), writer: Some(writer), signal, events, t0, halt: Some(halt), halted_at: None, failed, done })
    }

    /// The writer's error, the first time it is seen (the rest of this track is lost: the user should know now).
    /// Never waits: the supervisor does the device work.
    pub fn poll(&mut self) -> Option<String> {
        self.failed.try_recv().ok()
    }

    /// Stop capturing (call on every track first, so all tracks end at the same instant).
    pub fn halt(&mut self) {
        self.signal.stopping.store(true, Ordering::SeqCst);
        self.halt = None;
        self.halted_at.get_or_insert_with(Instant::now);
    }

    /// Close the track. Its info (events included) comes back even when the writer failed, with
    /// the error beside it. A device that doesn't let go within `LET_GO_WITHIN` is left to its supervisor: the
    /// file ends now.
    pub fn finish(mut self) -> (TrackInfo, Option<anyhow::Error>) {
        self.halt();
        let waited = self.halted_at.map(|t| t.elapsed()).unwrap_or_default();
        if self.done.recv_timeout(LET_GO_WITHIN.saturating_sub(waited)).is_err() {
            let at = self.t0.elapsed().as_secs_f64();
            let detail = format!("the device didn't let go within {} s: the track ends without it", LET_GO_WITHIN.as_secs());
            eprintln!("  [{at:7.1}s] {}: hung — {detail}", self.name);
            self.events.lock().unwrap_or_else(|p| p.into_inner()).push(TrackEvent { at, kind: "hung", detail });
            self.signal.halted.store(true, Ordering::SeqCst);
        }
        drop(self.tx.take());
        let written = self
            .writer
            .take()
            .expect("writer")
            .join()
            .map_err(|_| anyhow::anyhow!("{} writer thread panicked", self.name))
            .and_then(|r| r);
        // A failure the supervisor didn't see before it stopped.
        let failure = self.signal.failed.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let mut events = std::mem::take(&mut *self.events.lock().unwrap_or_else(|p| p.into_inner()));
        if let Some(e) = failure.filter(|_| !events.iter().any(|e| e.kind == "write_failed")) {
            events.push(TrackEvent { at: self.t0.elapsed().as_secs_f64(), kind: "write_failed", detail: e });
        }
        let ((frames, padded), err) = match written {
            Ok(n) => (n, None),
            Err(e) => ((0, 0), Some(e.context(format!("writing {}", self.file)))),
        };
        let info = TrackInfo { file: self.file.clone(), sample_rate: TRACK_RATE, frames, padded_frames: padded, events };
        (info, err)
    }
}

impl Supervisor {
    fn new(name: &'static str, source: Box<dyn Source>, tx: Sender<Chunk>, t0: Instant, signal: Arc<Signal>, events: Arc<Mutex<Vec<TrackEvent>>>) -> Self {
        Supervisor {
            name,
            source,
            live: None,
            epoch: 0,
            tx,
            t0,
            next_retry: Instant::now(),
            reopens: Vec::new(),
            last_error: None,
            events,
            restarts: 0,
            silent_restarts: 0,
            opened_at: Instant::now(),
            signal,
            watch_silence: name == "remote",
            write_failed: false,
        }
    }

    fn event(&mut self, kind: &'static str, detail: String) {
        let at = self.t0.elapsed().as_secs_f64();
        eprintln!("  [{at:7.1}s] {}: {kind} — {detail}", self.name);
        self.events.lock().unwrap_or_else(|p| p.into_inner()).push(TrackEvent { at, kind, detail });
    }

    fn try_open(&mut self) {
        if self.signal.stopping.load(Ordering::SeqCst) {
            return;
        }
        self.epoch += 1;
        match self.source.open(self.epoch, self.tx.clone()) {
            Ok(live) => {
                let opened_before = self.events.lock().unwrap_or_else(|p| p.into_inner()).iter().any(|e| e.kind == "capturing");
                let kind = if opened_before { "reopened" } else { "capturing" };
                self.event(kind, live.label());
                self.live = Some(live);
                self.last_error = None;
                self.opened_at = Instant::now();
            }
            Err(e) => {
                let msg = format!("{e:#}");
                if self.last_error.as_deref() != Some(&msg) {
                    self.event("waiting", msg.clone());
                    self.last_error = Some(msg);
                }
                self.next_retry = Instant::now() + RETRY_EVERY;
            }
        }
    }

    /// The writer's error, the first time it is seen (also a `write_failed` event): the rest of
    /// this track is lost, and the user should know now, not when the recording stops.
    fn write_failure(&mut self) -> Option<String> {
        if self.write_failed {
            return None;
        }
        let e = self.signal.failed.lock().unwrap_or_else(|p| p.into_inner()).clone()?;
        self.write_failed = true;
        self.event("write_failed", e.clone());
        Some(e)
    }

    /// Supervise: restart unhealthy streams, retry missing ones. Returns the writer's error the
    /// first time it is seen.
    fn poll(&mut self) -> Option<String> {
        let failed = self.write_failure();
        if self.live.is_some() && self.opened_at.elapsed() >= STABLE_RESETS {
            self.restarts = 0;
        }
        let health = match self.live.as_mut().map(|l| l.health()) {
            Some(Health::Ok) => self.silence_check(),
            // A planned change that keeps coming back is treated as a failure (backed off).
            Some(Health::Reopen(reason)) => {
                let now = Instant::now();
                self.reopens.retain(|t| now.duration_since(*t) < REOPEN_WINDOW);
                self.reopens.push(now);
                Some(if self.reopens.len() > REOPENS_PER_WINDOW { Health::Restart(format!("{reason} (again and again)")) } else { Health::Reopen(reason) })
            }
            other => other,
        };
        match health {
            Some(Health::Ok) => {}
            Some(Health::Restart(reason)) => {
                self.live = None; // stop the old incarnation first
                self.restarts += 1;
                self.event("restarting", reason);
                // Back off between consecutive restarts: 1 s, 2 s, 4 s … 30 s; slow after 20.
                let delay = if self.restarts > RESTARTS_BEFORE_SLOW {
                    BACKOFF_MAX
                } else {
                    Duration::from_secs(1 << self.restarts.saturating_sub(1).min(5)).min(BACKOFF_MAX)
                };
                if self.restarts <= 1 {
                    self.try_open();
                } else {
                    self.next_retry = Instant::now() + delay;
                }
            }
            Some(Health::Reopen(reason)) => {
                self.live = None;
                self.event("restarting", reason);
                self.try_open();
            }
            None if Instant::now() >= self.next_retry => self.try_open(),
            None => {}
        }
        failed
    }

    /// Audio keeps arriving but has been exact zeros for minutes: reopen (bounded).
    fn silence_check(&mut self) -> Option<Health> {
        if !self.watch_silence || self.silent_restarts >= SILENT_RESTARTS_MAX {
            return Some(Health::Ok);
        }
        let now_ms = self.t0.elapsed().as_millis() as u64;
        let opened_ms = now_ms.saturating_sub(self.opened_at.elapsed().as_millis() as u64);
        let chunk = self.signal.last_chunk_ms.load(Ordering::Relaxed);
        let sound = self.signal.last_sound_ms.load(Ordering::Relaxed).max(opened_ms);
        let limit = SILENT_RESTART_AFTER.as_millis() as u64;
        if now_ms.saturating_sub(chunk) < 2_000 && now_ms.saturating_sub(sound) >= limit {
            self.silent_restarts += 1;
            return Some(Health::Restart(format!("only digital silence for {} min", limit / 60_000)));
        }
        Some(Health::Ok)
    }
}

/// Writer thread: resample each epoch to TRACK_RATE and keep the file aligned to `t0`.
/// (frames written, of which padding)
type WriterHandle = JoinHandle<Result<(u64, u64)>>;

fn spawn_writer(
    path: &Path,
    t0: Instant,
    signal: Arc<Signal>,
) -> Result<(Sender<Chunk>, WriterHandle)> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TRACK_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let wav = hound::WavWriter::create(path, spec).with_context(|| format!("creating {}", path.display()))?;
    let (tx, rx) = mpsc::channel::<Chunk>();
    let handle = std::thread::spawn(move || {
        let r = write_track(rx, wav, t0, &signal);
        if let Err(e) = &r {
            // Said at once (`Track::poll`), not only when the recording stops.
            *signal.failed.lock().unwrap_or_else(|p| p.into_inner()) = Some(format!("{e:#}"));
        }
        r
    });
    Ok((tx, handle))
}

/// The writer's loop: chunks until the capture stops, then pad to "now" and finalize.
fn write_track(
    rx: Receiver<Chunk>,
    mut wav: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    t0: Instant,
    signal: &Signal,
) -> Result<(u64, u64)> {
    let (mut written, mut padded) = (0u64, 0u64);
    let mut epoch = 0u32;
    let mut rs = Resampler::new(TRACK_RATE, TRACK_RATE);
    let tolerance = (GAP_TOLERANCE.as_secs_f64() * TRACK_RATE as f64) as u64;
    loop {
        let chunk = match rx.recv_timeout(SUPERVISE_EVERY) {
            // The track ended without its device (it hung, maybe still sending): the file ends here.
            _ if signal.halted.load(Ordering::SeqCst) => break,
            Ok(c) => c,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if chunk.epoch != epoch || chunk.rate != rs.from {
            epoch = chunk.epoch;
            rs = Resampler::new(chunk.rate, TRACK_RATE);
        }
        let now_ms = t0.elapsed().as_millis() as u64;
        signal.last_chunk_ms.store(now_ms, Ordering::Relaxed);
        if chunk.samples.iter().any(|s| *s != 0.0) {
            signal.last_sound_ms.store(now_ms, Ordering::Relaxed);
        }
        let out = rs.process(&chunk.samples);
        // The chunk ends "now"; where should it start on the call timeline?
        let start = t0.elapsed().as_secs_f64() - chunk.samples.len() as f64 / chunk.rate as f64;
        let expected = (start.max(0.0) * TRACK_RATE as f64) as u64;
        if expected > written + tolerance {
            for _ in written..expected {
                wav.write_sample(0i16)?;
            }
            padded += expected - written;
            written = expected;
        }
        for s in &out {
            wav.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        written += out.len() as u64;
    }
    // Channel closed = capture stopped. Pad to "now" so every track spans the whole call
    // even if its source went quiet (an idle app's tap delivers nothing at all).
    let end = (t0.elapsed().as_secs_f64() * TRACK_RATE as f64) as u64;
    if end > written + tolerance {
        for _ in written..end {
            wav.write_sample(0i16)?;
        }
        padded += end - written;
        written = end;
    }
    wav.finalize()?;
    Ok((written, padded))
}

/// Fix the RIFF/data sizes of a WAV whose writer never finalized it (the app was killed
/// mid-recording): the samples are all there, only the header says 0. Returns whether it changed.
pub fn repair_wav(path: &Path) -> Result<bool> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut head = vec![0u8; len.min(4096) as usize];
    f.read_exact(&mut head)?;
    if head.len() < 12 || &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        anyhow::bail!("{} is not a WAV file", path.display());
    }
    // Walk the chunks to the "data" header.
    let mut pos = 12usize;
    while pos + 8 <= head.len() {
        let size = u32::from_le_bytes(head[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if &head[pos..pos + 4] == b"data" {
            let data_len = (len - (pos as u64 + 8)) & !1; // whole 16-bit samples
            let riff_len = len - 8;
            if size as u64 == data_len && u32::from_le_bytes(head[4..8].try_into().unwrap()) as u64 == riff_len {
                return Ok(false);
            }
            f.seek(SeekFrom::Start(4))?;
            f.write_all(&(riff_len.min(u32::MAX as u64) as u32).to_le_bytes())?;
            f.seek(SeekFrom::Start(pos as u64 + 4))?;
            f.write_all(&(data_len.min(u32::MAX as u64) as u32).to_le_bytes())?;
            return Ok(true);
        }
        pos += 8 + size + (size & 1);
    }
    anyhow::bail!("{}: no data chunk", path.display())
}

/// Streaming resampler for speech: box low-pass (when downsampling) + linear interpolation.
pub(crate) struct Resampler {
    from: u32,
    step: f64,
    pos: f64,
    hist: f32,
    lp: Vec<f32>,
    lp_i: usize,
    lp_sum: f32,
}

impl Resampler {
    pub(crate) fn new(from: u32, to: u32) -> Self {
        let step = from as f64 / to as f64;
        let taps = if step > 1.5 { step.round() as usize } else { 1 };
        Resampler { from, step, pos: 0.0, hist: 0.0, lp: vec![0.0; taps], lp_i: 0, lp_sum: 0.0 }
    }

    pub(crate) fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.step == 1.0 {
            return input.to_vec();
        }
        let n = self.lp.len();
        let x: Vec<f32> = input
            .iter()
            .map(|&s| {
                self.lp_sum += s - self.lp[self.lp_i];
                self.lp[self.lp_i] = s;
                self.lp_i = (self.lp_i + 1) % n;
                self.lp_sum / n as f32
            })
            .collect();
        if x.is_empty() {
            return vec![];
        }
        // Virtual signal: hist at index -1, then x[0..]. `pos` indexes into it.
        let mut out = Vec::with_capacity((x.len() as f64 / self.step) as usize + 1);
        let last = (x.len() - 1) as f64;
        while self.pos <= last {
            let i = self.pos.floor();
            let frac = (self.pos - i) as f32;
            let a = if i < 0.0 { self.hist } else { x[i as usize] };
            let b = x[(i + 1.0).min(last) as usize];
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        self.pos -= x.len() as f64;
        self.hist = x[x.len() - 1];
        out
    }
}

/// Average interleaved channels down to mono.
pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|f| f.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Source for the target app's audio output (the remote participants).
pub fn app_source(app: TargetApp) -> Box<dyn Source> {
    #[cfg(debug_assertions)]
    if let Some(s) = testfeed::remote() {
        return s;
    }
    #[cfg(target_os = "macos")]
    return Box::new(macos::AppTapSource::new(app));
    #[cfg(windows)]
    return Box::new(windows::LoopbackSource::new(app));
    #[cfg(not(any(target_os = "macos", windows)))]
    unimplemented!("app capture is only supported on macOS and Windows ({app:?})")
}

/// Source for the user's microphone: the mic the call app uses (`app`), or the default input (a memo: None).
pub fn mic_source(app: Option<TargetApp>) -> Box<dyn Source> {
    #[cfg(debug_assertions)]
    if let Some(s) = testfeed::me() {
        return s;
    }
    Box::new(mic::MicSource::new(app))
}

/// Is the app in a call right now (holding the mic)? None = this platform can't tell yet.
pub fn app_in_call(app: TargetApp) -> Option<bool> {
    #[cfg(debug_assertions)]
    if let Some(on) = testfeed::in_call() {
        return Some(on);
    }
    #[cfg(target_os = "macos")]
    return Some(macos::app_processes(app).map(|ps| ps.iter().any(|p| p.running_input)).unwrap_or(false));
    #[cfg(windows)]
    return Some(windows::app_in_call(app));
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = app;
        None
    }
}

/// The input device the call app is recording from, as (cpal device id, name). Following it
/// (rather than the system default) keeps headset users from being recorded via the laptop mic.
pub fn app_input_device(app: TargetApp) -> Option<(String, String)> {
    #[cfg(target_os = "macos")]
    return macos::app_input_device(app);
    #[cfg(windows)]
    return windows::app_input_device(app);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = app;
        None
    }
}

/// macOS only: 1.5 s global tap; returns the peak level (0.0 = silence or permission denied).
#[cfg(target_os = "macos")]
pub fn probe_system_audio() -> Result<f32> {
    macos::probe_system_audio()
}

pub fn print_audio_processes() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::print_audio_processes();
    #[cfg(windows)]
    return windows::print_audio_processes();
    #[cfg(not(any(target_os = "macos", windows)))]
    anyhow::bail!("unsupported platform")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn resampler_keeps_duration_across_chunks() {
        for from in [16_000, 24_000, 44_100, 48_000] {
            let mut rs = Resampler::new(from, TRACK_RATE);
            let mut total = 0;
            for _ in 0..100 {
                total += rs.process(&vec![0.5; from as usize / 100]).len();
            }
            // one second of input → one second of output, within a sample
            assert!((total as i64 - TRACK_RATE as i64).abs() <= 1, "{from}: {total}");
        }
    }

    /// A source that never opens (the track keeps waiting), for tests of the track itself.
    struct Never;
    impl Source for Never {
        fn open(&mut self, _epoch: u32, _tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
            anyhow::bail!("not playing")
        }
    }

    #[test]
    fn a_write_failure_is_reported_once_and_kept_in_the_tracks_events() {
        let (_env, root) = crate::paths::testenv::folders("track-write-failed");
        let t = std::cell::RefCell::new(Track::start("remote", &root.join("remote.wav"), Box::new(Never), Instant::now()).unwrap());
        assert_eq!(t.borrow_mut().poll(), None);
        // What the writer thread sets when a write fails; the track's supervisor reports it on its next look.
        *t.borrow().signal.failed.lock().unwrap() = Some("No space left on device".into());
        let seen = std::cell::RefCell::new(vec![]);
        crate::paths::testenv::eventually("the write failure is reported", || {
            seen.borrow_mut().extend(t.borrow_mut().poll());
            !seen.borrow().is_empty()
        });
        assert_eq!(seen.into_inner(), vec!["No space left on device".to_string()]);
        std::thread::sleep(SUPERVISE_EVERY * 2); // two more looks
        assert_eq!(t.borrow_mut().poll(), None, "once");
        let (info, err) = t.into_inner().finish();
        assert!(err.is_none(), "this writer itself was fine");
        assert_eq!(info.events.iter().filter(|e| e.kind == "write_failed").count(), 1);
        assert!(!info.events.iter().any(|e| e.kind == "hung"), "a track that lets go isn't hung");
    }

    /// A device that hangs until `free` is set, as a device listing or an open does while a Bluetooth headset
    /// reconnects: in its health check (`in_open` false) or in opening (true).
    struct Hanging {
        free: Arc<AtomicBool>,
        in_open: bool,
    }

    struct HangingStream(Arc<AtomicBool>);

    fn wait_until(free: &AtomicBool) {
        while !free.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    impl Source for Hanging {
        fn open(&mut self, _epoch: u32, _tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
            if self.in_open {
                wait_until(&self.free);
            }
            Ok(Box::new(HangingStream(self.free.clone())))
        }
    }

    impl LiveStream for HangingStream {
        fn label(&self) -> String {
            "hanging".into()
        }
        fn health(&mut self) -> Health {
            wait_until(&self.0);
            Health::Ok
        }
    }

    /// The device answers again after `secs`: a test that would wait on it ends anyway.
    fn free_after(secs: u64) -> Arc<AtomicBool> {
        let free = Arc::new(AtomicBool::new(false));
        let f = free.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            f.store(true, Ordering::SeqCst);
        });
        free
    }

    #[test]
    fn a_device_that_hangs_never_holds_up_the_track_or_its_end() {
        for in_open in [false, true] {
            let (_env, root) = crate::paths::testenv::folders("track-hangs");
            let free = free_after(4);
            let mut t = Track::start("mic", &root.join("mic.wav"), Box::new(Hanging { free: free.clone(), in_open }), Instant::now()).unwrap();
            std::thread::sleep(Duration::from_millis(600)); // the device work has begun, and hangs
            let at = Instant::now();
            for _ in 0..4 {
                assert_eq!(t.poll(), None);
            }
            assert!(at.elapsed() < Duration::from_millis(100), "in_open {in_open}: poll waited on the device for {:?}", at.elapsed());
            t.halt();
            let (info, err) = t.finish();
            assert!(at.elapsed() < Duration::from_millis(2500), "in_open {in_open}: the end waited on the device for {:?}", at.elapsed());
            assert!(err.is_none(), "{err:?}");
            assert!(info.events.iter().any(|e| e.kind == "hung"), "in_open {in_open}: {:?}", info.events.iter().map(|e| e.kind).collect::<Vec<_>>());
            assert!(hound::WavReader::open(root.join("mic.wav")).is_ok(), "the track's file is complete");
            free.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn resampler_preserves_dc_level() {
        let mut rs = Resampler::new(48_000, TRACK_RATE);
        let out = rs.process(&vec![0.25; 4800]);
        assert!(out[10..].iter().all(|s| (s - 0.25).abs() < 1e-4));
    }

    /// A source whose streams report the health a test scripts, counting how often it opens.
    struct FakeSource {
        opens: Arc<AtomicU64>,
        script: Arc<std::sync::Mutex<std::collections::VecDeque<Health>>>,
    }

    struct FakeStream {
        script: Arc<std::sync::Mutex<std::collections::VecDeque<Health>>>,
    }

    impl Source for FakeSource {
        fn open(&mut self, _epoch: u32, _tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeStream { script: self.script.clone() }))
        }
    }

    impl LiveStream for FakeStream {
        fn label(&self) -> String {
            "fake".into()
        }
        fn health(&mut self) -> Health {
            self.script.lock().unwrap().pop_front().unwrap_or(Health::Ok)
        }
    }

    #[test]
    fn a_supervisor_back_from_a_hang_after_the_stop_opens_nothing() {
        let opens = Arc::new(AtomicU64::new(0));
        let script = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from([Health::Restart("came back".into())])));
        let (tx, _rx) = mpsc::channel();
        let signal = Arc::new(Signal::default());
        let mut sup = Supervisor::new("mic", Box::new(FakeSource { opens: opens.clone(), script }), tx, Instant::now(), signal.clone(), Arc::new(Mutex::new(vec![])));
        sup.try_open();
        signal.stopping.store(true, Ordering::SeqCst); // the track was halted while the device hung
        sup.poll(); // the device answers: Restart
        assert_eq!(opens.load(Ordering::SeqCst), 1, "no device opened after the stop");
    }

    #[test]
    fn planned_reopens_skip_the_backoff_failures_dont() {
        let dir = std::env::temp_dir().join(format!("echo-track-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let opens = Arc::new(AtomicU64::new(0));
        let script = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new()));
        let source = FakeSource { opens: opens.clone(), script: script.clone() };
        let (tx, _rx) = mpsc::channel();
        let events = Arc::new(Mutex::new(vec![]));
        let mut track = Supervisor::new("mic", Box::new(source), tx, Instant::now(), Arc::new(Signal::default()), events);
        track.try_open();
        assert_eq!(opens.load(Ordering::SeqCst), 1);

        // Planned: every one reopens on the same poll, and none counts as a failure (up to three
        // within 30 s; more, flipping back and forth, back off like failures).
        for i in 0..REOPENS_PER_WINDOW as u64 {
            script.lock().unwrap().push_back(Health::Reopen(format!("device {i}")));
            track.poll();
            assert_eq!(opens.load(Ordering::SeqCst), 2 + i);
            assert!(track.live.is_some());
        }
        assert_eq!(track.restarts, 0);

        // A fourth within 30 s is flipping back and forth: it counts as a failure — the first
        // failure reopens at once, the next waits for the backoff.
        script.lock().unwrap().push_back(Health::Reopen("device again".into()));
        track.poll();
        assert_eq!(opens.load(Ordering::SeqCst), 5);
        assert_eq!(track.restarts, 1);
        script.lock().unwrap().push_back(Health::Restart("stalled".into()));
        track.poll();
        assert_eq!(opens.load(Ordering::SeqCst), 5);
        assert!(track.live.is_none());
        assert!(track.next_retry > Instant::now() + Duration::from_secs(1));
        assert_eq!(track.restarts, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod repair_tests {
    use super::*;

    #[test]
    fn unfinalized_wav_is_repaired() {
        let path = std::env::temp_dir().join(format!("echo-repair-{}.wav", std::process::id()));
        let spec = hound::WavSpec { channels: 1, sample_rate: TRACK_RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..16_000 {
            w.write_sample((i % 100) as i16).unwrap();
        }
        w.finalize().unwrap();
        // Simulate a killed writer: header sizes never filled in.
        let mut bytes = std::fs::read(&path).unwrap();
        let data = bytes.windows(4).position(|w| w == b"data").unwrap();
        bytes[4..8].copy_from_slice(&0u32.to_le_bytes());
        bytes[data + 4..data + 8].copy_from_slice(&0u32.to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        assert!(repair_wav(&path).unwrap());
        assert!(!repair_wav(&path).unwrap()); // idempotent
        let r = hound::WavReader::open(&path).unwrap();
        assert_eq!(r.len(), 16_000);
        let _ = std::fs::remove_file(&path);
    }
}
