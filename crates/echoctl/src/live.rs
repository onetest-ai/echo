//! Live transcription while a call is recorded: `<call>/live.jsonl`, one JSON line per
//! finished utterance (`speaker`, `start`, `end`, `lang`, `text`). The Echo mod draws it in
//! a pane beside the session. The final transcript is still made after the call with the
//! active (more accurate) model; this one favours speed: turbo when downloaded.
//!
//! Fed by the track writers with 16 kHz mono chunks at their position on the call timeline.
//! An utterance is decoded once it has been followed by 0.8 s of silence (or reaches 25 s).

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::config::Config;
use crate::meeting::MeetingContext;
use crate::models;
use crate::transcribe::{build_prompt, utterances, Transcriber, TrackState, RATE};

pub struct LiveChunk {
    pub speaker: &'static str,
    /// Sample index (16 kHz) of the first sample on the call timeline
    pub start: u64,
    pub samples: Vec<f32>,
}

pub const FILE: &str = "live.jsonl";
const LIVE_MODEL: &str = "large-v3-turbo-q5_0";
/// Chunks queued for the live worker (a few tens of seconds of audio).
const LIVE_QUEUE: usize = 4096;
/// At the end of a call, a backlog longer than this (30 s) is not decoded live.
const MAX_FLUSH_SAMPLES: usize = RATE * 30;
/// Silence after an utterance before it counts as finished.
const SETTLE: usize = RATE * 8 / 10;
/// Decode whatever is buffered once it gets this long, finished or not.
const FORCE: usize = RATE * 25;
const TICK: Duration = Duration::from_millis(1000);

struct Buf {
    base: u64,
    audio: Vec<f32>,
    state: TrackState,
}

/// Start the live worker for a call. None when no model is available (live view stays empty;
/// the post-call transcript is unaffected).
pub fn start(
    dir: &Path,
    meeting: Option<&MeetingContext>,
    languages: Option<Vec<String>>,
) -> Option<(SyncSender<LiveChunk>, JoinHandle<()>)> {
    let mut cfg = Config::load().ok()?;
    if let Some(l) = languages.filter(|l| !l.is_empty()) {
        cfg.languages = l; // pinned for this call
    }
    let live = models::find(LIVE_MODEL).ok().map(models::model_path).filter(|p| p.exists());
    let active = models::active_model_path().ok();
    let model = live.or(active)?;
    let prompt = build_prompt(&cfg, meeting);
    let remote_name = meeting.and_then(|m| m.sole_other(cfg.me.as_ref()));
    // Bounded: if decoding can't keep up, the writer drops live audio instead of the worker's
    // backlog growing without limit (the final transcript is unaffected).
    let (tx, rx) = mpsc::sync_channel(LIVE_QUEUE);
    let out = dir.join(FILE);
    let handle = std::thread::spawn(move || {
        if let Err(e) = run(&model, &cfg, prompt, rx, &out, remote_name) {
            eprintln!("live transcription stopped: {e:#}");
        }
    });
    Some((tx, handle))
}

fn run(
    model: &Path,
    cfg: &Config,
    prompt: Option<String>,
    rx: Receiver<LiveChunk>,
    out: &PathBuf,
    remote_name: Option<String>,
) -> anyhow::Result<()> {
    let tr = Transcriber::single(model, cfg, prompt)?;
    let mut bufs: HashMap<&'static str, Buf> = HashMap::new();
    let mut last = Instant::now();
    loop {
        let closed = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(c) => {
                let b = match bufs.get_mut(c.speaker) {
                    Some(b) => b,
                    None => {
                        let b = Buf { base: c.start, audio: vec![], state: tr.track_state()? };
                        bufs.entry(c.speaker).or_insert(b)
                    }
                };
                let end = b.base + b.audio.len() as u64;
                if c.start > end {
                    // the writer padded a gap: keep the timeline
                    b.audio.extend(std::iter::repeat_n(0.0, (c.start - end) as usize));
                }
                b.audio.extend_from_slice(&c.samples);
                false
            }
            Err(RecvTimeoutError::Timeout) => false,
            Err(RecvTimeoutError::Disconnected) => true,
        };
        if closed {
            // The call is over: decode the tail only if live kept up; a long backlog is
            // dropped (the final transcription starts now and covers everything).
            for b in bufs.values_mut() {
                if b.audio.len() > MAX_FLUSH_SAMPLES {
                    b.audio.clear();
                }
            }
        }
        if closed || last.elapsed() >= TICK {
            last = Instant::now();
            for (speaker, b) in bufs.iter_mut() {
                let name = if *speaker == "remote" { remote_name.as_deref() } else { None };
                drain(&tr, speaker, name, b, closed, out)?;
            }
        }
        if closed {
            return Ok(());
        }
    }
}

/// Decode the finished utterances at the front of the buffer and drop the audio they used.
fn drain(tr: &Transcriber, speaker: &str, name: Option<&str>, b: &mut Buf, flush: bool, out: &PathBuf) -> anyhow::Result<()> {
    let spans = utterances(&b.audio);
    let mut consumed = 0;
    for (s, e) in spans {
        let finished = flush || b.audio.len().saturating_sub(e) >= SETTLE || e - s >= FORCE;
        if !finished {
            break;
        }
        let (lang, segs) = tr.utterance(&mut b.state, &b.audio[s..e])?;
        let offset = (b.base + s as u64) as f64 / RATE as f64;
        let mut f = OpenOptions::new().create(true).append(true).open(out)?;
        for (st, en, text) in segs {
            let mut line = json!({ "speaker": speaker, "start": offset + st, "end": offset + en, "lang": lang, "text": text });
            if let Some(n) = name {
                line["name"] = json!(n);
            }
            writeln!(f, "{line}")?;
        }
        consumed = e;
    }
    if consumed == 0 && b.audio.len() > FORCE {
        // Long stretch with no speech: drop it, keeping a second for an utterance starting now.
        consumed = b.audio.len() - RATE;
    }
    if consumed > 0 {
        b.audio.drain(..consumed);
        b.base += consumed as u64;
    }
    Ok(())
}

/// Dev aid (`echoctl live-replay`): push a WAV through the live worker as the recorder would,
/// in 100 ms chunks, and print `live.jsonl` as it grows.
pub fn replay(wav: &Path, out_dir: &Path) -> anyhow::Result<()> {
    let mut r = hound::WavReader::open(wav)?;
    let spec = r.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect::<Result<_, _>>()?
        }
    };
    let mono = crate::audio::downmix(&raw, spec.channels as usize);
    let audio = crate::audio::Resampler::new(spec.sample_rate, RATE as u32).process(&mono);
    std::fs::create_dir_all(out_dir)?;
    let _ = std::fs::remove_file(out_dir.join(FILE));
    let (tx, h) = start(out_dir, None, None).ok_or_else(|| anyhow::anyhow!("no transcription model downloaded"))?;
    for (i, c) in audio.chunks(RATE / 10).enumerate() {
        tx.send(LiveChunk { speaker: "remote", start: (i * RATE / 10) as u64, samples: c.to_vec() })?;
        std::thread::sleep(Duration::from_millis(25)); // 4x real time
    }
    drop(tx);
    let _ = h.join();
    print!("{}", std::fs::read_to_string(out_dir.join(FILE)).unwrap_or_default());
    Ok(())
}
