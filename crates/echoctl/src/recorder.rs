//! A recording session: `<home>/calls/<timestamp>-<slug>/{remote.wav, mic.wav, meta.json}`.
//! Driven either by `echoctl capture` (manual) or by `echoctl watch` (automatic).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::Serialize;

use crate::apps::TargetApp;
use crate::audio::{self, mic::MicSource, Track, TrackInfo};
use crate::meeting::MeetingContext;
use crate::paths;

#[derive(Serialize)]
struct Meta {
    title: Option<String>,
    app: TargetApp,
    platform: &'static str,
    started_at: DateTime<Local>,
    ended_at: Option<DateTime<Local>>,
    /// Why the recording stopped (user, call ended, …)
    stop_reason: Option<String>,
    /// Languages pinned for this call (overrides the workspace list); absent = workspace list
    #[serde(skip_serializing_if = "Option::is_none")]
    languages: Option<Vec<String>>,
    /// The AI profile this call's live notes and minutes use (`ai.rs`), pinned at the start
    #[serde(skip_serializing_if = "Option::is_none")]
    ai_profile: Option<String>,
    tracks: Vec<TrackInfo>,
}

pub fn slugify(s: &str) -> String {
    let slug: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { '-' })
        .collect();
    let slug = slug.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    slug.chars().take(60).collect()
}

pub(crate) fn call_dir(started: &DateTime<Local>, title: Option<&str>) -> PathBuf {
    let mut name = started.format("%Y-%m-%d_%H%M%S").to_string();
    if let Some(t) = title.map(slugify).filter(|s| !s.is_empty()) {
        name.push('-');
        name.push_str(&t);
    }
    // Never reuse a folder (two recordings in the same second would truncate each other).
    let base = paths::calls_dir().join(&name);
    if !base.exists() {
        return base;
    }
    (2..).map(|n| paths::calls_dir().join(format!("{name}-{n}"))).find(|p| !p.exists()).unwrap_or(base)
}

/// The call's transcription worker (`live.rs`), when transcription is built in.
#[cfg(feature = "transcribe")]
pub type Live = crate::live::Live;
#[cfg(not(feature = "transcribe"))]
pub enum Live {}

pub struct Recording {
    live: Option<Live>,
    pub dir: PathBuf,
    pub t0: Instant,
    pub title: Option<String>,
    pub started_at: DateTime<Local>,
    meta: Meta,
    remote: Track,
    mic: Track,
}

impl Recording {
    pub fn start(
        app: TargetApp,
        title: Option<&str>,
        meeting: Option<&MeetingContext>,
        languages: Option<Vec<String>>,
        ai_profile: Option<String>,
    ) -> Result<Self> {
        let started_at = Local::now();
        // The meeting subject names the call when no explicit title is given.
        let title = title.map(str::to_owned).or_else(|| meeting.and_then(|m| m.subject.clone()));
        let dir = call_dir(&started_at, title.as_deref());
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let meta = Meta {
            title: title.clone(),
            app,
            platform: std::env::consts::OS,
            started_at,
            ended_at: None,
            stop_reason: None,
            languages: languages.clone(),
            ai_profile,
            tracks: vec![],
        };
        paths::write_atomic(&dir.join("meta.json"), &serde_json::to_vec_pretty(&meta)?)?;
        if let Some(m) = meeting {
            m.save(&dir)?;
        }
        // Both tracks share t0, so their timelines line up sample-for-sample.
        let t0 = Instant::now();
        let remote = Track::start("remote", &dir.join("remote.wav"), audio::app_source(app), t0)?;
        let mic = Track::start("mic", &dir.join("mic.wav"), Box::new(MicSource { app: Some(app) }), t0)?;
        // Transcribes the tracks as they are written (reads the WAVs, so it starts after them).
        #[cfg(feature = "transcribe")]
        let live = crate::live::start(&dir, meeting, languages);
        #[cfg(not(feature = "transcribe"))]
        let live = {
            let _ = languages;
            None
        };
        Ok(Recording {
            live,
            dir,
            t0,
            title,
            started_at,
            meta,
            remote,
            mic,
        })
    }

    /// Switch the call's AI profile mid-call. The recording owns `meta.json` until it finishes
    /// (and rewrites it whole then), so the change goes through here.
    pub fn set_ai_profile(&mut self, id: &str) -> Result<()> {
        let _lock = crate::calls::lock_meta(&self.dir)?;
        let before = self.meta.ai_profile.replace(id.to_string());
        if let Err(e) = paths::write_atomic(&self.dir.join("meta.json"), &serde_json::to_vec_pretty(&self.meta)?) {
            self.meta.ai_profile = before; // memory and disk say the same
            return Err(e.into());
        }
        Ok(())
    }

    /// Supervise both tracks; call every ~250 ms. Returns each track whose audio can no longer
    /// be written (its name, why), once.
    pub fn poll(&mut self) -> Vec<(&'static str, String)> {
        let remote = self.remote.poll().map(|e| ("remote", e));
        let mic = self.mic.poll().map(|e| ("mic", e));
        remote.into_iter().chain(mic).collect()
    }

    /// Close the tracks. Returns the call's folder, its tracks and the transcription worker
    /// (now finishing the last utterances; `Live::join` waits for it), if one ran.
    ///
    /// A failure doesn't cut it short: both tracks are closed, the worker is told to stop and
    /// `meta.json` gets its end, then the first error comes back (the worker, dropped, stops).
    pub fn finish(self, reason: &str) -> Result<(PathBuf, Vec<TrackInfo>, Option<Live>)> {
        let Recording { live, dir, mut meta, mut remote, mut mic, .. } = self;
        // Stop both captures at the same instant so the tracks end together.
        remote.halt();
        mic.halt();
        let mut errors = vec![];
        for track in [remote, mic] {
            let (info, err) = track.finish();
            if let Some(e) = err {
                // A writer that died never wrote the WAV's sizes: fix them, the samples are there.
                // Its failure was told when it happened (`Track::poll`) and is in the track's
                // events: the recording itself closes normally, and what was saved is used.
                let _ = audio::repair_wav(&dir.join(&info.file));
                eprintln!("{}: {e:#}", info.file);
            }
            meta.tracks.push(info);
        }
        // The WAVs are complete: the worker decodes what is left. Not joined here: a slow
        // decoder must not hold up the watcher, the tray or the heartbeat.
        #[cfg(feature = "transcribe")]
        if let Some(l) = &live {
            l.close();
        }
        meta.ended_at = Some(Local::now());
        meta.stop_reason = Some(reason.to_string());
        if let Err(e) = write_final_meta(&dir, &mut meta) {
            errors.push(e);
        }
        match errors.into_iter().next() {
            Some(e) => Err(e),
            None => Ok((dir, meta.tracks, live)),
        }
    }
}

/// The recording's last `meta.json`, under the call's lock.
fn write_final_meta(dir: &std::path::Path, meta: &mut Meta) -> Result<()> {
    let _lock = crate::calls::lock_meta(dir)?;
    // A profile switch written straight to the file is kept: it can reach a recording that
    // has just started, before the heartbeat says it records (`ai::set_call_profile`).
    let on_disk = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    if let Some(id) = on_disk.as_ref().and_then(|m| m["ai_profile"].as_str()).filter(|s| !s.is_empty()) {
        meta.ai_profile = Some(id.to_string());
    }
    paths::write_atomic(&dir.join("meta.json"), &serde_json::to_vec_pretty(meta)?)?;
    Ok(())
}

/// `echoctl capture`: record until Ctrl-C or `--seconds`.
pub fn capture(
    app: TargetApp,
    seconds: Option<u64>,
    title: Option<&str>,
    meeting: Option<MeetingContext>,
    languages: Option<Vec<String>>,
) -> Result<()> {
    let ai_profile = Some(crate::ai::pin(None, meeting.as_ref()));
    let mut rec = Recording::start(app, title, meeting.as_ref(), languages, ai_profile)?;
    eprintln!("● recording {app:?} + microphone → {}", rec.dir.display());
    eprintln!("  press Ctrl-C to stop; device switches are followed automatically");

    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    ctrlc::set_handler(move || s.store(true, Ordering::SeqCst))?;
    while !stop.load(Ordering::SeqCst) && seconds.is_none_or(|n| rec.t0.elapsed() < Duration::from_secs(n)) {
        std::thread::sleep(Duration::from_millis(250));
        // A track that can't be written any more is said by its event, on stderr.
        let _ = rec.poll();
    }

    // `capture` leaves transcription to `echoctl transcribe` (the worker ends with the process).
    let (dir, tracks, _live) = rec.finish(if stop.load(Ordering::SeqCst) { "user" } else { "duration" })?;
    for t in &tracks {
        let restarts = t.events.iter().filter(|e| e.kind == "restarting").count();
        eprintln!(
            "  {} — {:.1}s, {:.1}s padded, {restarts} restart(s)",
            t.file,
            t.frames as f64 / t.sample_rate as f64,
            t.padded_frames as f64 / t.sample_rate as f64
        );
    }
    println!("{}", dir.display());
    Ok(())
}
