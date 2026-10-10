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

fn call_dir(started: &DateTime<Local>, title: Option<&str>) -> PathBuf {
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

pub struct Recording {
    #[cfg(feature = "transcribe")]
    live: Option<std::thread::JoinHandle<()>>,
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
            tracks: vec![],
        };
        fs::write(dir.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
        if let Some(m) = meeting {
            m.save(&dir)?;
        }
        // Both tracks share t0, so their timelines line up sample-for-sample.
        let t0 = Instant::now();
        #[cfg(feature = "transcribe")]
        let (feed, live) = match crate::live::start(&dir, meeting, languages) {
            Some((tx, h)) => (Some(tx), Some(h)),
            None => (None, None),
        };
        #[cfg(not(feature = "transcribe"))]
        let feed: Option<audio::LiveFeed> = None;
        let remote = Track::start("remote", &dir.join("remote.wav"), audio::app_source(app), t0, feed.clone())?;
        let mic = Track::start("mic", &dir.join("mic.wav"), Box::new(MicSource { app: Some(app) }), t0, feed)?;
        Ok(Recording {
            #[cfg(feature = "transcribe")]
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

    /// Supervise both tracks; call every ~250 ms.
    pub fn poll(&mut self) {
        self.remote.poll();
        self.mic.poll();
    }

    pub fn finish(mut self, reason: &str) -> Result<(PathBuf, Vec<TrackInfo>)> {
        // Stop both captures at the same instant so the tracks end together.
        self.remote.halt();
        self.mic.halt();
        self.meta.tracks.push(self.remote.finish()?);
        self.meta.tracks.push(self.mic.finish()?);
        // The tracks closed their live feeds; the live worker finishes on its own (not
        // joined: a slow decoder must not hold up the watcher, the tray or the heartbeat).
        #[cfg(feature = "transcribe")]
        drop(self.live.take());
        self.meta.ended_at = Some(Local::now());
        self.meta.stop_reason = Some(reason.to_string());
        fs::write(self.dir.join("meta.json"), serde_json::to_vec_pretty(&self.meta)?)?;
        Ok((self.dir, self.meta.tracks))
    }
}

/// `echoctl capture`: record until Ctrl-C or `--seconds`.
pub fn capture(
    app: TargetApp,
    seconds: Option<u64>,
    title: Option<&str>,
    meeting: Option<MeetingContext>,
    languages: Option<Vec<String>>,
) -> Result<()> {
    let mut rec = Recording::start(app, title, meeting.as_ref(), languages)?;
    eprintln!("● recording {app:?} + microphone → {}", rec.dir.display());
    eprintln!("  press Ctrl-C to stop; device switches are followed automatically");

    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    ctrlc::set_handler(move || s.store(true, Ordering::SeqCst))?;
    while !stop.load(Ordering::SeqCst) && seconds.is_none_or(|n| rec.t0.elapsed() < Duration::from_secs(n)) {
        std::thread::sleep(Duration::from_millis(250));
        rec.poll();
    }

    let (dir, tracks) = rec.finish(if stop.load(Ordering::SeqCst) { "user" } else { "duration" })?;
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
