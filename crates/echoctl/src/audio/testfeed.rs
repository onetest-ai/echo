//! Functional tests only, in a debug build: a call fed from files, so a test records no device and plays no sound.
//!
//! - `ECHOCTL_TEST_ME_WAV=<wav>` replaces the microphone source (the mic track);
//! - `ECHOCTL_TEST_REMOTE_WAV=<wav>` replaces the call app's audio tap (the remote track);
//! - `ECHOCTL_TEST_CALL_FILE=<path>` replaces call detection: the call app is in a call while that file exists, and
//!   the call started at its modification time (`tests/fixtures/call.py`);
//! - while `<that path>.stall` exists, every test stream's health check blocks, as a device listing does while a
//!   Bluetooth headset reconnects.
//!
//! Everything after the source is the real code: the tracks, their writers and alignment, live transcription and
//! notes, the watcher's start and stop, the files written. The wavs are read when a recording opens its tracks, in
//! real time against the call's clock: a recording that starts mid-call gets only what follows, as from a device.
//! After the file's end the source sends nothing (the writer pads the track to the stop, as for an idle tap). A call
//! that starts again (a drop and rejoin: the call file made anew) reopens the sources on the new files and clock.
//! A release build never reads any of this.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};

use super::{downmix, Chunk, Health, LiveStream, Source};

const ME: &str = "ECHOCTL_TEST_ME_WAV";
const REMOTE: &str = "ECHOCTL_TEST_REMOTE_WAV";
const CALL: &str = "ECHOCTL_TEST_CALL_FILE";
/// How often the feeder sends what the clock has reached (a device delivers every 10–20 ms).
const TICK: Duration = Duration::from_millis(20);

fn var(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The mic source fed from `ECHOCTL_TEST_ME_WAV`, when it is set.
pub fn me() -> Option<Box<dyn Source>> {
    var(ME).map(|path| Box::new(FileSource::new(path)) as Box<dyn Source>)
}

/// The remote source fed from `ECHOCTL_TEST_REMOTE_WAV`, when it is set.
pub fn remote() -> Option<Box<dyn Source>> {
    var(REMOTE).map(|path| Box::new(FileSource::new(path)) as Box<dyn Source>)
}

/// With `ECHOCTL_TEST_CALL_FILE` set: whether the call app is in a call (the file exists). None when unset.
pub fn in_call() -> Option<bool> {
    var(CALL).map(|p| p.exists())
}

/// When the current call started: the call file's modification time, if there is one.
fn call_started() -> Option<SystemTime> {
    std::fs::metadata(var(CALL)?).and_then(|m| m.modified()).ok()
}

/// A wav as mono samples and its rate.
fn read_wav(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("test audio {}", path.display()))?;
    let spec = r.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|s| s as f32 / scale)).collect::<Result<_, _>>()?
        }
    };
    Ok((downmix(&interleaved, spec.channels as usize), spec.sample_rate))
}

/// A wav played against the call's clock.
pub struct FileSource {
    path: PathBuf,
    /// Without a call file (`echoctl capture`): the clock starts at the first open, and a reopen goes on from there.
    first_open: Option<SystemTime>,
}

impl FileSource {
    pub fn new(path: PathBuf) -> Self {
        FileSource { path, first_open: None }
    }
}

impl Source for FileSource {
    fn open(&mut self, epoch: u32, tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
        let (samples, rate) = read_wav(&self.path)?;
        let call = call_started();
        let start = call.unwrap_or(*self.first_open.get_or_insert_with(SystemTime::now));
        let stop = Arc::new(AtomicBool::new(false));
        feed(samples, rate, start, epoch, tx, stop.clone());
        Ok(Box::new(FileStream { label: format!("test audio {} @ {rate} Hz", self.path.display()), stop, call }))
    }
}

/// Send `samples` as the clock started at `start` reaches them, from where it is now: what was "said" before the
/// open is skipped, as a device would have missed it.
fn feed(samples: Vec<f32>, rate: u32, start: SystemTime, epoch: u32, tx: Sender<Chunk>, stop: Arc<AtomicBool>) {
    let len = samples.len();
    let at = move |offset: Duration| ((offset.as_secs_f64() * rate as f64) as usize).min(len);
    let offset = SystemTime::now().duration_since(start).unwrap_or_default();
    let opened = Instant::now();
    std::thread::spawn(move || {
        let mut sent = at(offset);
        while sent < len && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(TICK);
            let due = at(offset + opened.elapsed());
            if due > sent {
                if tx.send(Chunk { epoch, rate, samples: samples[sent..due].to_vec() }).is_err() {
                    return;
                }
                sent = due;
            }
        }
    });
}

struct FileStream {
    label: String,
    stop: Arc<AtomicBool>,
    /// The call this stream plays against (the call file's time when it opened).
    call: Option<SystemTime>,
}

impl LiveStream for FileStream {
    fn label(&self) -> String {
        self.label.clone()
    }

    fn health(&mut self) -> Health {
        while stalled() {
            std::thread::sleep(Duration::from_millis(100));
        }
        if rejoined(self.call, call_started()) {
            return Health::Reopen("the test call started again".into());
        }
        Health::Ok
    }
}

impl Drop for FileStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// A device that hangs: `<call file>.stall` is there.
fn stalled() -> bool {
    var(CALL).is_some_and(|p| std::path::PathBuf::from(format!("{}.stall", p.display())).exists())
}

/// A new call since the stream opened: the call file is there again with another start. Not a hang-up (no file).
fn rejoined(opened: Option<SystemTime>, now: Option<SystemTime>) -> bool {
    now.is_some() && now != opened
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("echo-testfeed-{}-{name}", std::process::id()))
    }

    fn wav(path: &Path, samples: &[i16], rate: u32) {
        let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for s in samples {
            w.write_sample(*s).unwrap();
        }
        w.finalize().unwrap();
    }

    fn collect(rx: std::sync::mpsc::Receiver<Chunk>) -> Vec<f32> {
        let mut got = vec![];
        while let Ok(c) = rx.recv_timeout(Duration::from_secs(2)) {
            assert_eq!((c.epoch, c.rate), (7, 8000));
            got.extend(c.samples);
        }
        got
    }

    #[test]
    fn a_file_source_sends_the_whole_wav_in_real_time() {
        let path = temp("me.wav");
        let samples: Vec<i16> = (0..2400).map(|i| (i % 100) as i16 * 100).collect(); // 0.3 s at 8 kHz
        wav(&path, &samples, 8000);
        let (tx, rx) = std::sync::mpsc::channel();
        let t0 = Instant::now();
        let live = FileSource::new(path.clone()).open(7, tx).unwrap();
        assert!(live.label().contains("me.wav"));
        let got = collect(rx);
        assert!(t0.elapsed() >= Duration::from_millis(280), "fed in real time, not at once");
        let want: Vec<f32> = samples.iter().map(|s| *s as f32 / 32768.0).collect();
        assert_eq!(got, want);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_source_opened_mid_call_skips_what_was_said_before() {
        let path = temp("remote.wav");
        let samples: Vec<i16> = (0..4000).map(|i| i as i16).collect(); // 0.5 s at 8 kHz
        wav(&path, &samples, 8000);
        let (samples, rate) = read_wav(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let (tx, rx) = std::sync::mpsc::channel();
        // The call started 0.25 s ago: the first 2000 samples are gone.
        feed(samples, rate, SystemTime::now() - Duration::from_millis(250), 7, tx, Arc::new(AtomicBool::new(false)));
        let got = collect(rx);
        assert!((1900..=2000).contains(&(4000 - got.len())), "skipped {}", 4000 - got.len());
        assert_eq!(*got.last().unwrap(), 3999.0 / 32768.0);
    }

    #[test]
    fn only_a_call_started_again_reopens_the_source() {
        let t = SystemTime::now();
        let later = t + Duration::from_secs(40);
        assert!(rejoined(Some(t), Some(later)), "a rejoin: the call file made anew");
        assert!(rejoined(None, Some(t)), "a call starting after a capture opened without one");
        assert!(!rejoined(Some(t), Some(t)), "the same call");
        assert!(!rejoined(Some(t), None), "a hang-up: the file is gone");
        assert!(!rejoined(None, None), "no call file at all (`echoctl capture`)");
    }

    #[test]
    fn a_missing_test_file_is_an_error_that_names_it() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let e = FileSource::new(PathBuf::from("/nonexistent/me.wav")).open(1, tx).err().unwrap();
        assert!(format!("{e:#}").contains("/nonexistent/me.wav"));
    }
}
