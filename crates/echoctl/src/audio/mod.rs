//! Audio capture.
//!
//! A *Source* knows how to open a live capture (a mic stream, a process tap). A *Track*
//! supervises one source: it polls the live stream's health and rebuilds it whenever the
//! device, sample rate or app processes change, so the user can switch headphones or mics
//! mid-call. Every incarnation streams mono `Chunk`s to the track's writer thread, which
//! resamples to a fixed rate and pads gaps with silence against a shared wall clock — so all
//! tracks of a call stay aligned no matter how often a source is rebuilt.

pub mod mic;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
pub(crate) mod windows;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
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
    /// Tear down and reopen, for the given reason.
    Restart(String),
}

/// One running capture. Dropping it stops the capture.
pub trait LiveStream {
    /// Human-readable description of what is being captured (device / processes, rate).
    fn label(&self) -> String;
    /// Called every poll (~250 ms).
    fn health(&mut self) -> Health;
}

pub trait Source {
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
/// Restart policy: back off 1 s → 30 s between restarts;
/// beyond 20 restarts only retry every 30 s; a stream stable for 5 min resets the count.
const BACKOFF_MAX: Duration = Duration::from_secs(30);
const RESTARTS_BEFORE_SLOW: u32 = 20;
const STABLE_RESETS: Duration = Duration::from_secs(5 * 60);
/// Audio arriving as exact digital zeros this long means a dead stream (or a revoked
/// permission, which macOS signals with zeros): reopen it, at most this many times.
const SILENT_RESTART_AFTER: Duration = Duration::from_secs(5 * 60);
const SILENT_RESTARTS_MAX: u32 = 10;

/// What the writer saw: ms since t0 of the last chunk, and of the last non-zero chunk.
#[derive(Default)]
struct Signal {
    last_chunk_ms: AtomicU64,
    last_sound_ms: AtomicU64,
}

pub struct Track {
    name: &'static str,
    source: Box<dyn Source>,
    live: Option<Box<dyn LiveStream>>,
    epoch: u32,
    tx: Option<Sender<Chunk>>,
    writer: Option<WriterHandle>,
    file: String,
    t0: Instant,
    next_retry: Instant,
    last_error: Option<String>,
    events: Vec<TrackEvent>,
    restarts: u32,
    silent_restarts: u32,
    opened_at: Instant,
    signal: Arc<Signal>,
    /// Reopen when the stream delivers only zeros for too long (the call app's track).
    watch_silence: bool,
}

impl Track {
    /// Start a track. If the source can't open yet (e.g. Teams isn't playing audio), the track
    /// keeps retrying every second instead of failing.
    pub fn start(name: &'static str, path: &Path, source: Box<dyn Source>, t0: Instant) -> Result<Self> {
        let signal = Arc::new(Signal::default());
        let (tx, writer) = spawn_writer(path, t0, signal.clone())?;
        let mut t = Track {
            name,
            source,
            live: None,
            epoch: 0,
            tx: Some(tx),
            writer: Some(writer),
            file: path.file_name().unwrap().to_string_lossy().into_owned(),
            t0,
            next_retry: Instant::now(),
            last_error: None,
            events: vec![],
            restarts: 0,
            silent_restarts: 0,
            opened_at: Instant::now(),
            signal,
            watch_silence: name == "remote",
        };
        t.try_open();
        Ok(t)
    }

    fn event(&mut self, kind: &'static str, detail: String) {
        let at = self.t0.elapsed().as_secs_f64();
        eprintln!("  [{at:7.1}s] {}: {kind} — {detail}", self.name);
        self.events.push(TrackEvent { at, kind, detail });
    }

    fn try_open(&mut self) {
        self.epoch += 1;
        let tx = self.tx.clone().expect("track finished");
        match self.source.open(self.epoch, tx) {
            Ok(live) => {
                let kind = if self.events.iter().any(|e| e.kind == "capturing") { "reopened" } else { "capturing" };
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

    /// Supervise: restart unhealthy streams, retry missing ones.
    pub fn poll(&mut self) {
        if self.live.is_some() && self.opened_at.elapsed() >= STABLE_RESETS {
            self.restarts = 0;
        }
        let health = match self.live.as_mut().map(|l| l.health()) {
            Some(Health::Ok) => self.silence_check(),
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
            None if Instant::now() >= self.next_retry => self.try_open(),
            None => {}
        }
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

    /// Stop capturing (call on every track first, so all tracks end at the same instant).
    pub fn halt(&mut self) {
        self.live = None;
    }

    pub fn finish(mut self) -> Result<TrackInfo> {
        self.live = None;
        drop(self.tx.take());
        let (frames, padded) = self
            .writer
            .take()
            .expect("writer")
            .join()
            .map_err(|_| anyhow::anyhow!("{} writer thread panicked", self.name))??;
        Ok(TrackInfo {
            file: self.file.clone(),
            sample_rate: TRACK_RATE,
            frames,
            padded_frames: padded,
            events: std::mem::take(&mut self.events),
        })
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
    let mut wav = hound::WavWriter::create(path, spec).with_context(|| format!("creating {}", path.display()))?;
    let (tx, rx) = mpsc::channel::<Chunk>();
    let handle = std::thread::spawn(move || -> Result<(u64, u64)> {
        let (mut written, mut padded) = (0u64, 0u64);
        let mut epoch = 0u32;
        let mut rs = Resampler::new(TRACK_RATE, TRACK_RATE);
        let tolerance = (GAP_TOLERANCE.as_secs_f64() * TRACK_RATE as f64) as u64;
        for chunk in rx {
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
    });
    Ok((tx, handle))
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
    #[cfg(target_os = "macos")]
    return Box::new(macos::AppTapSource::new(app));
    #[cfg(windows)]
    return Box::new(windows::LoopbackSource::new(app));
    #[cfg(not(any(target_os = "macos", windows)))]
    unimplemented!("app capture is only supported on macOS and Windows ({app:?})")
}

/// Is the app in a call right now (holding the mic)? None = this platform can't tell yet.
pub fn app_in_call(app: TargetApp) -> Option<bool> {
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

    #[test]
    fn resampler_preserves_dc_level() {
        let mut rs = Resampler::new(48_000, TRACK_RATE);
        let out = rs.process(&vec![0.25; 4800]);
        assert!(out[10..].iter().all(|s| (s - 0.25).abs() < 1e-4));
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
