//! Transcription while a call is recorded — the call's only pass. A worker reads `mic.wav` and
//! `remote.wav` as the track writers grow them (the WAVs on disk are the queue: nothing is
//! dropped when decoding falls behind, the text just arrives later) and decodes each finished
//! utterance with the final pass's transcriber (the configured model, turbo for language ID
//! beside a model other than turbo until the call's one language is decided, the vocabulary-hint guard). Every line goes to
//! `<call>/live.jsonl` (`speaker`, `start`, `end`, `lang`, `text`), which the Echo mod draws in
//! its pane; when the call ends the worker
//! decodes what is left and hands back all segments, and the watcher writes the transcript —
//! seconds after the call instead of a second pass over the whole recording.
//!
//! On speakers the mic re-records the others. Once they have talked for a while (`FIND_AFTER`), the worker looks for
//! that echo in what is recorded so far; found by two looks in a row, it holds for the call, and the stretches of the
//! mic it explains along their whole length aren't transcribed (`aec::Echo::keep`): the user's own words, a short "yes"
//! inside the echo or talking over the others, still are. Until it is found every utterance is transcribed, as on
//! headphones. Found and skipped during the call, and the call's last minutes show no other echo (`TAIL`), nothing is
//! redone at the end; otherwise, if the whole call shows an echo, the mic track is echo-cancelled and decoded again at
//! the end, as before.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::json;

use crate::aec;
use crate::config::Config;
use crate::meeting::MeetingContext;
use crate::models;
use crate::transcribe::{build_prompt, final_transcriber, load_16k, utterances, Segment, TrackState, Transcriber, RATE};

pub const FILE: &str = "live.jsonl";
/// Silence after an utterance before it counts as finished.
const SETTLE: usize = RATE * 8 / 10;
/// Decode whatever is buffered once it gets this long, finished or not.
const FORCE: usize = RATE * 25;
/// Read at most this much audio ahead of the decoder (the rest waits on disk).
const MAX_BUFFER: usize = RATE * 60;
const TICK: Duration = Duration::from_millis(1000);
/// whisper.cpp threads during a call: keeping up is all it needs (Metal does the work on
/// macOS), and the call itself needs the machine too.
const THREADS: usize = 4;
/// The other side has talked this long before the echo is looked for during the call: 15 s of its loud frames (`aec`'s
/// talking ones), about 30 s of it talking. It is looked for again after every `FIND_EVERY` more of the call (15 s)
/// until it is found.
const FIND_AFTER: usize = 15_000 / 30;
const FIND_EVERY: usize = 15_000 / 30;
/// At the end, the echo found is checked against the call's last 3 minutes: a changed one (the volume, another device)
/// wasn't skipped since, and the mic is decoded again.
const TAIL: usize = 180_000 / 30;
/// How long `Live::abandon` waits for the worker to let go of its model.
const ABANDON_WAIT: Duration = Duration::from_secs(30);

/// The worker's thread: every segment of the call and the model's name.
type Worker = JoinHandle<Result<(Vec<Segment>, String)>>;

/// The worker of one recording. Dropped without `join` (nobody will use its transcript: the
/// call is transcribed from its WAVs instead), it is told to give up at once — not to decode
/// what is left — so it never keeps polling, or decoding beside that pass, with its model loaded.
pub struct Live {
    done: Arc<AtomicBool>,
    quit: Arc<AtomicBool>,
    handle: Option<Worker>,
}

impl Live {
    /// The tracks are complete on disk: decode what is left and stop.
    pub fn close(&self) {
        self.done.store(true, Ordering::SeqCst);
    }

    /// Wait for the worker: every segment of the call and the model's name.
    pub fn join(mut self) -> Result<(Vec<Segment>, String)> {
        self.close();
        let handle = self.handle.take().expect("joined once");
        handle.join().map_err(|_| anyhow!("the transcription worker panicked"))?
    }
}

impl Live {
    /// Give up and wait until the worker has let go of its model: a process about to exit must not leave a
    /// whisper context alive (ggml's Metal device asserts at exit while one is, #141).
    /// Waited for at most `ABANDON_WAIT`: a local decode ends within it (the worker checks `quit` between
    /// utterances), a request to a transcription server may not — and holds no local model unless the server
    /// failed over to one mid-call, which the wait then covers as well. Past the cap the worker is left running
    /// (detached): a local model still loaded then can still trip ggml's Metal assert at exit (#141), a rare case
    /// traded for an exit that never waits more than the cap.
    pub fn abandon(mut self) {
        self.quit.store(true, Ordering::SeqCst);
        self.close();
        let Some(h) = self.handle.take() else { return };
        let until = std::time::Instant::now() + ABANDON_WAIT;
        while !h.is_finished() && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        if h.is_finished() {
            let _ = h.join();
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        // Joined: done already. Not joined: abandoned.
        if self.handle.is_some() {
            self.quit.store(true, Ordering::SeqCst);
        }
        self.close();
    }
}

/// Whether live transcription can run here: a model here, or a transcription server to send
/// the audio to.
fn can_transcribe() -> bool {
    crate::remote::Client::configured().is_some() || models::active_model_path().ok().is_some_and(|p| p.exists())
}

/// Start the worker for a call. None when no model is downloaded and no server is set (the call is then transcribed
/// after it ends, as before).
pub fn start(dir: &Path, meeting: Option<&MeetingContext>) -> Option<Live> {
    let cfg = Config::load().ok()?;
    if !can_transcribe() {
        return None;
    }
    let prompt = build_prompt(&cfg, meeting);
    let remote_name = meeting.and_then(|m| m.sole_other(cfg.me.as_ref()));
    let (done, quit) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let (d, q, dir) = (done.clone(), quit.clone(), dir.to_path_buf());
    let handle = std::thread::spawn(move || {
        let r = run(&dir, &cfg, prompt, remote_name, &d, &q);
        if let Err(e) = &r {
            eprintln!("transcription during the call stopped: {e:#}");
        }
        r
    });
    Some(Live { done, quit, handle: Some(handle) })
}

/// One track's WAV, read as it grows.
struct TrackReader {
    path: PathBuf,
    speaker: &'static str,
    /// Byte offset of the samples (16-bit mono at 16 kHz, as the track writer makes them).
    data_at: Option<u64>,
    /// Samples read from the file so far.
    read: u64,
    /// Call-timeline sample index of `audio[0]`.
    base: u64,
    audio: Vec<f32>,
    /// The last read reached the end of what is on disk.
    caught_up: bool,
    state: TrackState,
}

/// Where a WAV's samples start: after the `data` chunk header (its size is still 0 while the
/// writer is open).
fn data_offset(f: &mut File) -> Result<Option<u64>> {
    let mut head = vec![0u8; 4096];
    let n = f.read(&mut head)?;
    let head = &head[..n];
    if n < 12 || &head[..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Ok(None);
    }
    let mut pos = 12;
    while pos + 8 <= head.len() {
        let size = u32::from_le_bytes(head[pos + 4..pos + 8].try_into()?) as usize;
        if &head[pos..pos + 4] == b"data" {
            return Ok(Some(pos as u64 + 8));
        }
        pos += 8 + size + size % 2;
    }
    Ok(None)
}

/// Up to `max` samples of a track's WAV (as the writer has it so far) from sample `from`; `data_at` found once.
fn read_samples(path: &Path, data_at: &mut Option<u64>, from: u64, max: usize) -> Result<Vec<f32>> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    if data_at.is_none() {
        *data_at = data_offset(&mut f)?;
    }
    let Some(at) = *data_at else { return Ok(vec![]) };
    f.seek(SeekFrom::Start(at + from * 2))?;
    let mut bytes = Vec::with_capacity(max * 2);
    f.take(max as u64 * 2).read_to_end(&mut bytes)?;
    Ok(bytes.as_chunks::<2>().0.iter().map(|&b| i16::from_le_bytes(b) as f32 / i16::MAX as f32).collect())
}

impl TrackReader {
    fn new(path: PathBuf, speaker: &'static str, state: TrackState) -> Self {
        TrackReader { path, speaker, data_at: None, read: 0, base: 0, audio: vec![], caught_up: false, state }
    }

    /// Read what the writer has added since last time (up to `MAX_BUFFER` buffered).
    fn fill(&mut self) -> Result<()> {
        self.caught_up = true;
        let room = MAX_BUFFER.saturating_sub(self.audio.len());
        if room == 0 {
            self.caught_up = false;
            return Ok(());
        }
        let samples = read_samples(&self.path, &mut self.data_at, self.read, room)?;
        let n = samples.len();
        self.audio.extend(samples);
        self.read += n as u64;
        self.caught_up = n < room;
        Ok(())
    }
}

/// A track's loudness (`aec::envelope`, whole frames) from the start of the call, read from its WAV apart from the
/// decoder's buffer: it reaches what is on disk however far behind the decoding is.
struct Loudness {
    path: PathBuf,
    data_at: Option<u64>,
    read: u64,
    env: Vec<f32>,
}

impl Loudness {
    fn new(path: PathBuf) -> Self {
        Loudness { path, data_at: None, read: 0, env: vec![] }
    }

    fn fill(&mut self) -> Result<()> {
        const CHUNK: usize = aec::ENV * 1000;
        loop {
            let s = read_samples(&self.path, &mut self.data_at, self.read, CHUNK)?;
            let whole = s.len() / aec::ENV * aec::ENV;
            self.env.extend(aec::envelope(&s[..whole]));
            self.read += whole as u64;
            if s.len() < CHUNK {
                return Ok(());
            }
        }
    }
}

/// The speakers' echo on the mic, looked for during the call (module doc) and, once found, what of the mic it explains.
struct EchoWatch {
    mic: Loudness,
    remote: Loudness,
    found: Option<aec::Echo>,
    /// What the last look found, not yet confirmed: the echo is found when two looks in a row agree (`Echo::same_as`).
    candidate: Option<aec::Echo>,
    /// Envelope frames there were at the last look.
    looked: usize,
    /// Mic samples not transcribed.
    skipped: u64,
}

impl EchoWatch {
    fn new(dir: &Path) -> Self {
        EchoWatch { mic: Loudness::new(dir.join("mic.wav")), remote: Loudness::new(dir.join("remote.wav")), found: None, candidate: None, looked: 0, skipped: 0 }
    }

    /// Read what the tracks have added; not found yet, look for the echo every `FIND_EVERY`.
    fn update(&mut self) -> Result<()> {
        self.mic.fill()?;
        self.remote.fill()?;
        self.look();
        Ok(())
    }

    fn look(&mut self) {
        let n = self.mic.env.len().min(self.remote.env.len());
        if self.found.is_none() && n >= self.looked + FIND_EVERY {
            self.looked = n;
            let look = aec::find(&self.mic.env[..n], &self.remote.env[..n], FIND_AFTER);
            match (self.candidate, look) {
                (Some(c), Some(e)) if e.same_as(&c) => self.found = Some(e),
                _ => self.candidate = look,
            }
            if let Some(e) = &self.found {
                eprintln!("  echo found {:.0} s into the call: {} ms, gain {:.2}, r {:.2} — the mic's stretches it explains aren't transcribed",
                    n as f64 * aec::ENV as f64 / RATE as f64, e.delay_ms(), e.gain, e.r);
            }
        }
    }

    /// The parts of the mic's samples `start..end` (on the call's timeline) to transcribe: all of them until the echo
    /// is found.
    fn keep(&mut self, start: u64, end: u64) -> Vec<(u64, u64)> {
        let Some(echo) = self.found else { return vec![(start, end)] };
        let f = aec::ENV as u64;
        let (from, to) = ((start / f) as usize, end.div_ceil(f) as usize);
        let parts: Vec<(u64, u64)> = echo
            .keep(&self.mic.env, &self.remote.env, from, to)
            .into_iter()
            .map(|(a, b)| (if a == from { start } else { a as u64 * f }, if b == to { end } else { b as u64 * f }))
            .collect();
        self.skipped += (end - start) - parts.iter().map(|(a, b)| b - a).sum::<u64>();
        parts
    }

    /// The call's echo was dealt with while it was transcribed: found, some of the mic skipped, and the call's last
    /// `TAIL` shows no other echo.
    fn handled(&self) -> bool {
        let Some(e) = self.found.filter(|_| self.skipped > 0) else { return false };
        let n = self.mic.env.len().min(self.remote.env.len());
        let from = n.saturating_sub(TAIL);
        match aec::find(&self.mic.env[from..n], &self.remote.env[from..n], 0) {
            Some(last) if !last.same_as(&e) => {
                eprintln!("  echo changed during the call: {} ms, gain {:.2} at the end — decoding the mic again", last.delay_ms(), last.gain);
                false
            }
            _ => true,
        }
    }
}

/// The mic is echo-cancelled and decoded again at the end: echo cancellation on, and the echo not dealt with during
/// the call (`EchoWatch::handled`).
fn redo_at_stop(watch: Option<&EchoWatch>) -> bool {
    watch.is_some_and(|w| !w.handled())
}

/// Where in the buffer (which starts at sample `base` of the call) each part of an utterance is, and its time in
/// the call (s).
fn slices(base: u64, parts: &[(u64, u64)]) -> Vec<(f64, std::ops::Range<usize>)> {
    parts.iter().map(|&(a, b)| (a as f64 / RATE as f64, (a - base) as usize..(b - base) as usize)).collect()
}

fn run(dir: &Path, cfg: &Config, prompt: Option<String>, remote_name: Option<String>, done: &AtomicBool, quit: &AtomicBool) -> Result<(Vec<Segment>, String)> {
    let (tr, model) = final_transcriber(cfg, None, prompt, THREADS, Some(dir))?;
    let out = dir.join(FILE);
    let mut tracks = [
        TrackReader::new(dir.join("mic.wav"), "me", tr.track_state()?),
        TrackReader::new(dir.join("remote.wav"), "remote", tr.track_state()?),
    ];
    let mut segs = vec![];
    let mut watch = cfg.echo_cancel.unwrap_or(true).then(|| EchoWatch::new(dir));
    // The call's language changed by hand during it (`call_language::set`): the rest in it, the whole call again at
    // the end.
    let mut relanguaged = false;
    loop {
        if let Some(l) = crate::call_language::meta_language(dir).filter(|l| *l != tr.language()) {
            tr.set_language(l);
            relanguaged = true;
        }
        // Read before draining: once `done` is seen, what is on disk is the whole track.
        let last = done.load(Ordering::SeqCst);
        let mut behind = false;
        for t in tracks.iter_mut() {
            let name = if t.speaker == "remote" { remote_name.as_deref() } else { None };
            loop {
                if quit.load(Ordering::SeqCst) {
                    bail!("given up: nobody waits for this transcript");
                }
                t.fill()?;
                // The mic's utterances are judged against the echo with both tracks read at least as far as this one.
                let echo = match watch.as_mut() {
                    Some(w) if t.speaker == "me" => {
                        w.update()?;
                        Some(w)
                    }
                    _ => None,
                };
                drain(&tr, t, name, last && t.caught_up, &out, &mut segs, echo)?;
                if !last || t.caught_up {
                    break;
                }
            }
            behind |= !t.caught_up;
        }
        if last {
            break;
        }
        if !behind {
            std::thread::sleep(TICK);
        }
    }
    // Given up while the last pass ran (`abandon` after `close`): no echo cancellation, no second decode.
    if quit.load(Ordering::SeqCst) {
        bail!("given up: nobody waits for this transcript");
    }
    // On speakers the mic re-records the others. Skipped during the call: done. Otherwise cancel that echo and decode
    // the mic again (headphones — no echo found — keep what was decoded during the call).
    if let Some((w, e)) = watch.as_ref().and_then(|w| Some((w, w.found?))) {
        eprintln!("  echo: {} ms, gain {:.2}, r {:.2}; skipped {:.1} s of the mic", e.delay_ms(), e.gain, e.r, w.skipped as f64 / RATE as f64);
    }
    let (mic, remote) = (dir.join("mic.wav"), dir.join("remote.wav"));
    if relanguaged && mic.exists() && remote.exists() {
        eprintln!("  the call's language was changed during it: transcribing all of it again");
        segs.retain(|s| s.speaker != "remote");
        let mut theirs = tr.track("remote", &load_16k(&remote)?)?;
        if let Some(n) = remote_name.as_deref() {
            theirs.iter_mut().for_each(|s| s.name = Some(n.to_string()));
        }
        segs.extend(theirs);
        if !redo_at_stop(watch.as_ref()) {
            segs.retain(|s| s.speaker != "me");
            segs.extend(tr.track("me", &load_16k(&mic)?)?);
        }
    }
    if (redo_at_stop(watch.as_ref()) || relanguaged && watch.is_some()) && mic.exists() && remote.exists() {
        match crate::aec::cancel(&load_16k(&mic)?, &load_16k(&remote)?) {
            Ok(Some((clean, delay))) => {
                eprintln!("  echo cancellation: on (speaker → mic delay ≈ {delay} ms) — decoding the mic again");
                segs.retain(|s| s.speaker != "me");
                segs.extend(tr.track("me", &clean)?);
            }
            Ok(None) => {}
            Err(e) => eprintln!("  echo cancellation skipped: {e:#}"),
        }
    }
    Ok((segs, if relanguaged { tr.model_name() } else { model }))
}

/// Decode the finished utterances at the front of the buffer, drop the audio they used, and
/// append their lines to `live.jsonl`. `flush`: the buffer's end is the end of the track.
/// `echo`: the mic's; the parts of an utterance it explains aren't decoded.
fn drain(tr: &Transcriber, t: &mut TrackReader, name: Option<&str>, flush: bool, out: &Path, segs: &mut Vec<Segment>, mut echo: Option<&mut EchoWatch>) -> Result<()> {
    let spans = utterances(&t.audio);
    let mut consumed = 0;
    for &(s, e) in &spans {
        let finished = flush || t.audio.len().saturating_sub(e) >= SETTLE || e - s >= FORCE;
        if !finished {
            break;
        }
        let (start, end) = (t.base + s as u64, t.base + e as u64);
        let parts = echo.as_deref_mut().map_or_else(|| vec![(start, end)], |w| w.keep(start, end));
        for (offset, part) in slices(t.base, &parts) {
            let (lang, decoded) = tr.utterance(&mut t.state, &t.audio[part])?;
            let mut f = OpenOptions::new().create(true).append(true).open(out)?;
            for p in decoded {
                let seg = Segment::from_piece(t.speaker, offset, &lang, p, name);
                writeln!(f, "{}", json!(seg))?;
                segs.push(seg);
            }
        }
        consumed = e;
    }
    if flush {
        consumed = t.audio.len();
    } else if consumed == 0 && t.audio.len() > FORCE {
        // A long buffer with nothing finished: drop the silence before the speech still going
        // on (all of it when there is none, keeping a second for an utterance starting now).
        // Never the speech itself: a fast engine reads the buffer while a 20 s utterance is
        // still growing after a pause.
        consumed = spans.first().map_or(t.audio.len() - RATE, |s| s.0);
    }
    if consumed > 0 {
        t.audio.drain(..consumed);
        t.base += consumed as u64;
    }
    Ok(())
}

/// The mark `live-replay` leaves in its output folder: only a folder holding it (or a new or
/// empty one) is written over — never a call's.
const REPLAY_MARK: &str = ".replay";
const REPLAY_MARK_TEXT: &str = "echoctl live-replay\n";

/// Refuse an output folder that isn't a replay's: anything there but nothing, or a folder this
/// command marked. Anything that can't be read refuses (fail closed).
fn replay_folder_ok(out_dir: &Path) -> Result<()> {
    let refuse = |why: &str| anyhow!("{} {why}; live-replay writes only into a new or empty folder, or one it made", out_dir.display());
    match std::fs::symlink_metadata(out_dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(refuse(&format!("can't be read ({e})"))),
        Ok(m) if !m.is_dir() => return Err(refuse("isn't a folder")),
        Ok(_) => {}
    }
    let mut entries = std::fs::read_dir(out_dir).map_err(|e| refuse(&format!("can't be read ({e})")))?;
    if entries.next().is_none() {
        return Ok(());
    }
    match std::fs::symlink_metadata(out_dir.join(REPLAY_MARK)) {
        Ok(m) if m.is_file() => {}
        Ok(_) => return Err(refuse("has a .replay that isn't a file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(refuse("isn't empty and isn't a replay's")),
        Err(e) => return Err(refuse(&format!("can't be read ({e})"))),
    }
    match std::fs::read_to_string(out_dir.join(REPLAY_MARK)) {
        Ok(t) if t == REPLAY_MARK_TEXT => Ok(()),
        Ok(_) => Err(refuse("has a .replay this command didn't write")),
        Err(e) => Err(refuse(&format!("has a .replay that can't be read ({e})"))),
    }
}

/// Dev aid (`echoctl live-replay`): play a WAV into `<out>/remote.wav` as a recording would
/// (growing on disk, 10× real time) while the worker transcribes it, then close it and write the
/// transcript as the watcher does after a call (the audio is kept).
pub fn replay(wav: &Path, out_dir: &Path) -> Result<()> {
    let audio = load_16k(wav)?;
    // Nothing is touched before the folder is known to be a replay's and transcription can run.
    replay_folder_ok(out_dir)?;
    if Config::load().is_err() || !can_transcribe() {
        return Err(anyhow!("no transcription model downloaded (or server connected)"));
    }
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(out_dir.join(REPLAY_MARK), REPLAY_MARK_TEXT)?;
    for f in [FILE, "transcript.md", "segments.json"] {
        match std::fs::remove_file(out_dir.join(f)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(anyhow!("can't remove {}: {e}", out_dir.join(f).display()));
            }
            _ => {}
        }
    }
    if !out_dir.join("meta.json").exists() {
        std::fs::write(out_dir.join("meta.json"), json!({ "title": "replay", "started_at": chrono::Local::now().to_rfc3339() }).to_string())?;
    }
    let spec = hound::WavSpec { channels: 1, sample_rate: RATE as u32, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(out_dir.join("remote.wav"), spec)?;
    let t = std::time::Instant::now();
    let live = start(out_dir, None).ok_or_else(|| anyhow!("no transcription model downloaded"))?;
    for c in audio.chunks(RATE / 10) {
        for s in c {
            w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        w.flush()?;
        std::thread::sleep(Duration::from_millis(10));
    }
    w.finalize()?;
    let played = t.elapsed().as_secs_f64();
    let lines = std::fs::read_to_string(out_dir.join(FILE)).unwrap_or_default().lines().count();
    let (segs, model) = live.join()?;
    let after = t.elapsed().as_secs_f64() - played;
    let md = crate::transcribe::write_transcript(out_dir, &Config::load()?, &model, segs, "", true)?;
    eprintln!("{:.0}s of audio played in {played:.0}s ({lines} lines live by then); transcript {:.1}s after the end: {}",
        audio.len() as f64 / RATE as f64, after, md.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_worker_is_told_to_stop() {
        let done = Arc::new(AtomicBool::new(false));
        let d = done.clone();
        let handle = std::thread::spawn(move || {
            while !d.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok((vec![], String::new()))
        });
        let quit = Arc::new(AtomicBool::new(false));
        drop(Live { done: done.clone(), quit: quit.clone(), handle: Some(handle) });
        assert!(done.load(Ordering::SeqCst) && quit.load(Ordering::SeqCst), "a worker nobody joins gives up");
        // And `join` still works (it takes the handle; its drop then only sets the flag again).
        let d = Arc::new(AtomicBool::new(false));
        let d2 = d.clone();
        let q = Arc::new(AtomicBool::new(false));
        let live = Live { done: d, quit: q.clone(), handle: Some(std::thread::spawn(move || {
            while !d2.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok((vec![], "m".into()))
        })) };
        assert_eq!(live.join().unwrap().1, "m");
        assert!(!q.load(Ordering::SeqCst), "a joined worker isn't told to give up");
    }

    #[test]
    fn an_abandoned_worker_on_a_local_model_is_waited_for_until_it_lets_go() {
        let (done, quit, gone) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
        let (q, g) = (quit.clone(), gone.clone());
        let handle = std::thread::spawn(move || {
            while !q.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            std::thread::sleep(Duration::from_millis(50)); // still freeing its model
            g.store(true, Ordering::SeqCst);
            Ok((vec![], String::new()))
        });
        Live { done: done.clone(), quit: quit.clone(), handle: Some(handle) }.abandon();
        assert!(quit.load(Ordering::SeqCst) && done.load(Ordering::SeqCst));
        assert!(gone.load(Ordering::SeqCst), "abandon returns only after the worker ended (#141)");
    }

    #[test]
    fn reads_samples_of_a_wav_still_being_written() {
        let dir = std::env::temp_dir().join(format!("echo-live-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.wav");
        let spec = hound::WavSpec { channels: 1, sample_rate: 16_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..1000 {
            w.write_sample(i as i16).unwrap();
        }
        w.flush().unwrap(); // on disk, header not finalized
        let mut f = File::open(&path).unwrap();
        let at = data_offset(&mut f).unwrap().unwrap();
        let mut bytes = vec![];
        f.seek(SeekFrom::Start(at)).unwrap();
        f.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes.len(), 2000);
        assert_eq!(i16::from_le_bytes([bytes[2 * 999], bytes[2 * 999 + 1]]), 999);
        drop(w);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The first run of at least `min` frames where `talks` and not `quiet`, after frame `after`: its samples, with a
    /// quarter second around it (as the VAD pads an utterance).
    fn turn(talks: &[f32], quiet: &[f32], after: usize, min: usize) -> (u64, u64) {
        let (mut k, f) = (after, aec::ENV as u64);
        loop {
            let run = (k..talks.len()).take_while(|&i| talks[i] > 0.0 && quiet[i] == 0.0).count();
            if run >= min {
                return (k as u64 * f - RATE as u64 / 4, (k + run) as u64 * f + RATE as u64 / 4);
            }
            k += run + 1;
        }
    }

    #[test]
    fn the_echo_is_skipped_only_once_found_and_then_not_redone_at_the_end() {
        use crate::aec::signals::{turn_taking, with_echo};
        let dir = std::env::temp_dir().join(format!("echo-live-echo-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Laptop speakers: the mic hears the other side 0.35× 40 ms late.
        let n = RATE * 150;
        let (user, remote) = turn_taking(7, n);
        let mic = with_echo(&user, &remote, 0.35, 640);
        let spec = hound::WavSpec { channels: 1, sample_rate: RATE as u32, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let (mut wm, mut wr) = (hound::WavWriter::create(dir.join("mic.wav"), spec).unwrap(), hound::WavWriter::create(dir.join("remote.wav"), spec).unwrap());
        let mut write = |a: usize, b: usize| {
            for i in a..b {
                wm.write_sample((mic[i] * i16::MAX as f32) as i16).unwrap();
                wr.write_sample((remote[i] * i16::MAX as f32) as i16).unwrap();
            }
            wm.flush().unwrap();
            wr.flush().unwrap();
        };
        let (eu, er) = (aec::envelope(&user), aec::envelope(&remote));
        let (early_echo, late_echo, late_user) = (turn(&er, &eu, 100, 33), turn(&er, &eu, 4000, 33), turn(&eu, &er, 4000, 33));

        // 25 s in, the other side has talked under 15 s: no decision yet (though the echo already shows), and every
        // utterance is transcribed.
        write(0, RATE * 25);
        let mut w = EchoWatch::new(&dir);
        w.update().unwrap();
        assert!(w.found.is_none() && aec::find(&w.mic.env, &w.remote.env, 0).is_some());
        assert_eq!(w.keep(early_echo.0, early_echo.1), vec![early_echo], "nothing skipped before the echo is found");
        assert!(!w.handled(), "not found: the end decides on the whole call, as before");

        // Then every 15 s: one look that finds it is only a candidate, the next that agrees finds it.
        let mut at = RATE * 25;
        while at < n {
            let next = (at + RATE * 15).min(n);
            write(at, next);
            let had = w.candidate.is_some();
            w.update().unwrap();
            assert!(w.found.is_none() || had, "found on a single look at {} s", next / RATE);
            at = next;
        }
        // The whole call: found, and the echo of a turn of the other side isn't transcribed; the user's own turn is.
        let e = w.found.expect("the echo is found during the call");
        assert!((30..=60).contains(&e.delay_ms()), "{e:?}");
        assert_eq!(w.keep(late_user.0, late_user.1), vec![late_user]);
        assert!(!w.handled(), "found, but nothing skipped yet: the end still cancels the echo and decodes again");
        assert_eq!(w.keep(late_echo.0, late_echo.1), vec![]);
        assert_eq!(w.skipped, late_echo.1 - late_echo.0);
        assert!(w.handled(), "found and skipped: nothing is decoded again at the end");
        assert!(!redo_at_stop(Some(&w)) && !redo_at_stop(None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_echo_that_changed_since_it_was_found_is_cancelled_again_at_the_end() {
        use crate::aec::signals::{turn_taking, with_echo};
        let dir = std::env::temp_dir().join(format!("echo-live-echo-change-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 0.35× for the first 2 minutes, then the volume turned up: 0.7× for the last 4.
        let n = RATE * 360;
        let (user, remote) = turn_taking(11, n);
        let (before, after) = (with_echo(&user, &remote, 0.35, 640), with_echo(&user, &remote, 0.7, 640));
        let mic: Vec<f32> = (0..n).map(|i| if i < RATE * 120 { before[i] } else { after[i] }).collect();
        let spec = hound::WavSpec { channels: 1, sample_rate: RATE as u32, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let (mut wm, mut wr) = (hound::WavWriter::create(dir.join("mic.wav"), spec).unwrap(), hound::WavWriter::create(dir.join("remote.wav"), spec).unwrap());
        let mut w = EchoWatch::new(&dir);
        for step in (0..n).step_by(RATE * 15) {
            for i in step..(step + RATE * 15).min(n) {
                wm.write_sample((mic[i] * i16::MAX as f32) as i16).unwrap();
                wr.write_sample((remote[i] * i16::MAX as f32) as i16).unwrap();
            }
            wm.flush().unwrap();
            wr.flush().unwrap();
            w.update().unwrap();
            if step + RATE * 15 == RATE * 120 {
                // Before the change: found at 0.35×, and an echo turn skipped.
                let e = w.found.expect("found in the first 2 minutes");
                assert!((0.3..0.4).contains(&e.gain), "{e:?}");
                let (eu, er) = (aec::envelope(&user), aec::envelope(&remote));
                let echo_turn = turn(&er, &eu, 1000, 33);
                assert_eq!(w.keep(echo_turn.0, echo_turn.1), vec![]);
                assert!(w.handled());
            }
        }
        assert!(!w.handled() && redo_at_stop(Some(&w)), "the echo at the end isn't the one found: the mic is decoded again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headphones_never_get_an_echo_during_the_call() {
        // 30 five-minute calls on headphones with a room's noise, the user now and then talking over the other side:
        // looked at every 15 s, never an echo.
        use crate::aec::signals::{talk_over, turn_taking, with_noise};
        for seed in 1..=30u32 {
            let n = RATE * 300;
            let (mut user, remote) = turn_taking(seed, n);
            talk_over(&mut user, &remote, seed + 100);
            let (em, er) = (aec::envelope(&with_noise(&user, seed)), aec::envelope(&remote));
            let mut w = EchoWatch::new(Path::new("/nonexistent"));
            for k in (FIND_EVERY..=em.len()).step_by(FIND_EVERY) {
                w.mic.env = em[..k].to_vec();
                w.remote.env = er[..k].to_vec();
                w.look();
            }
            assert_eq!(w.found, None, "seed {seed}: {:?}", w.candidate);
        }
    }

    #[test]
    fn one_look_alone_finds_no_echo() {
        // A look that sees an echo by chance (#250 review: r 0.38 on headphones) is only a candidate: the next look
        // (`FIND_EVERY` later) must see the same echo.
        use crate::aec::signals::{turn_taking, with_echo};
        let (user, remote) = turn_taking(5, RATE * 120);
        let (echoed, eu, er) = (aec::envelope(&with_echo(&user, &remote, 0.35, 640)), aec::envelope(&user), aec::envelope(&remote));
        let mut w = EchoWatch::new(Path::new("/nonexistent"));
        let mut look = |mic: &[f32], k: usize| {
            (w.mic.env, w.remote.env) = (mic[..k].to_vec(), er[..k].to_vec());
            w.look();
            (w.found, w.candidate)
        };
        assert!(matches!(look(&echoed, 2000), (None, Some(_))), "one look: a candidate");
        assert_eq!(look(&eu, 2500), (None, None), "the next sees no echo: nothing found, the candidate dropped");
        assert!(matches!(look(&echoed, 3000), (None, Some(_))));
        assert!(matches!(look(&echoed, 3500), (Some(_), _)), "two looks in a row that agree: found");
    }

    #[test]
    fn the_parts_of_an_utterance_are_decoded_from_their_own_place_and_time() {
        // A buffer starting 10 s into the call; an utterance's two parts kept around a skipped stretch.
        let base = RATE as u64 * 10;
        let parts = [(base + 8000, base + 16_000), (base + 40_000, base + 48_000)];
        assert_eq!(slices(base, &parts), vec![(10.5, 8000..16_000), (12.5, 40_000..48_000)]);
    }

    #[test]
    fn replay_writes_only_into_its_own_folders() {
        let root = std::env::temp_dir().join(format!("echo-replay-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        // New or empty: fine.
        assert!(replay_folder_ok(&root.join("new")).is_ok());
        let empty = root.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(replay_folder_ok(&empty).is_ok());
        // A call's folder: refused, and nothing in it touched — also by `replay` itself.
        let call = root.join("2026-10-23_090000-x");
        std::fs::create_dir_all(&call).unwrap();
        std::fs::write(call.join("meta.json"), r#"{"title":"replay"}"#).unwrap();
        std::fs::write(call.join("transcript.md"), "# words").unwrap();
        assert!(replay_folder_ok(&call).is_err());
        let wav = root.join("in.wav");
        let spec = hound::WavSpec { channels: 1, sample_rate: 16_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&wav, spec).unwrap();
        for _ in 0..1600 {
            w.write_sample(0i16).unwrap();
        }
        w.finalize().unwrap();
        let e = replay(&wav, &call).unwrap_err().to_string();
        assert!(e.contains("isn't empty"), "{e}");
        assert_eq!(std::fs::read_to_string(call.join("transcript.md")).unwrap(), "# words");
        // A mark of another text, or not a file: refused; the command's own mark: fine.
        std::fs::write(call.join(REPLAY_MARK), "x").unwrap();
        assert!(replay_folder_ok(&call).is_err());
        std::fs::write(call.join(REPLAY_MARK), REPLAY_MARK_TEXT).unwrap();
        assert!(replay_folder_ok(&call).is_ok());
        let odd = root.join("odd");
        std::fs::create_dir_all(odd.join(REPLAY_MARK)).unwrap();
        assert!(replay_folder_ok(&odd).is_err());
        // A file given as the folder: refused.
        assert!(replay_folder_ok(&wav).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
