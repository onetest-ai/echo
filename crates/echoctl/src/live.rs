//! Transcription while a call is recorded — the call's only pass. A worker reads `mic.wav` and
//! `remote.wav` as the track writers grow them (the WAVs on disk are the queue: nothing is
//! dropped when decoding falls behind, the text just arrives later) and decodes each finished
//! utterance with the final pass's transcriber (the configured model, turbo for language ID,
//! the vocabulary-hint guard). Every line goes to `<call>/live.jsonl` (`speaker`, `start`,
//! `end`, `lang`, `text`), which the Echo mod draws in its pane; when the call ends the worker
//! decodes what is left and hands back all segments, and the watcher writes the transcript —
//! seconds after the call instead of a second pass over the whole recording.
//!
//! One step still needs the whole call: whether the user was on speakers (the mic re-recording
//! the others). If so, the mic track is echo-cancelled and decoded again at the end; on
//! headphones nothing is redone.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::json;

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
pub fn start(dir: &Path, meeting: Option<&MeetingContext>, languages: Option<Vec<String>>) -> Option<Live> {
    let mut cfg = Config::load().ok()?;
    if let Some(l) = languages.filter(|l| !l.is_empty()) {
        cfg.languages = l; // pinned for this call
    }
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
        let mut f = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if self.data_at.is_none() {
            self.data_at = data_offset(&mut f)?;
        }
        let Some(at) = self.data_at else { return Ok(()) };
        f.seek(SeekFrom::Start(at + self.read * 2))?;
        let mut bytes = Vec::with_capacity(room * 2);
        f.take(room as u64 * 2).read_to_end(&mut bytes)?;
        let n = bytes.len() / 2;
        self.audio.extend(bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32));
        self.read += n as u64;
        self.caught_up = n < room;
        Ok(())
    }
}

fn run(dir: &Path, cfg: &Config, prompt: Option<String>, remote_name: Option<String>, done: &AtomicBool, quit: &AtomicBool) -> Result<(Vec<Segment>, String)> {
    let (tr, model) = final_transcriber(cfg, None, prompt, THREADS)?;
    let out = dir.join(FILE);
    let mut tracks = [
        TrackReader::new(dir.join("mic.wav"), "me", tr.track_state()?),
        TrackReader::new(dir.join("remote.wav"), "remote", tr.track_state()?),
    ];
    let mut segs = vec![];
    loop {
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
                drain(&tr, t, name, last && t.caught_up, &out, &mut segs)?;
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
    // On speakers the mic re-records the others: cancel that echo and decode the mic again
    // (headphones — no echo found — keep what was decoded during the call).
    let (mic, remote) = (dir.join("mic.wav"), dir.join("remote.wav"));
    if cfg.echo_cancel.unwrap_or(true) && mic.exists() && remote.exists() {
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
    Ok((segs, model))
}

/// Decode the finished utterances at the front of the buffer, drop the audio they used, and
/// append their lines to `live.jsonl`. `flush`: the buffer's end is the end of the track.
fn drain(tr: &Transcriber, t: &mut TrackReader, name: Option<&str>, flush: bool, out: &Path, segs: &mut Vec<Segment>) -> Result<()> {
    let spans = utterances(&t.audio);
    let mut consumed = 0;
    for &(s, e) in &spans {
        let finished = flush || t.audio.len().saturating_sub(e) >= SETTLE || e - s >= FORCE;
        if !finished {
            break;
        }
        let (lang, decoded) = tr.utterance(&mut t.state, &t.audio[s..e])?;
        let offset = (t.base + s as u64) as f64 / RATE as f64;
        let mut f = OpenOptions::new().create(true).append(true).open(out)?;
        for p in decoded {
            let seg = Segment::from_piece(t.speaker, offset, &lang, p, name);
            writeln!(f, "{}", json!(seg))?;
            segs.push(seg);
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
    let live = start(out_dir, None, None).ok_or_else(|| anyhow!("no transcription model downloaded"))?;
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
