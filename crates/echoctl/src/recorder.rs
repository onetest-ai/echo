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
use crate::audio::{self, Track, TrackInfo};
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
    /// The call's language as a list, for older readers of `meta.json`: `[language]`
    #[serde(skip_serializing_if = "Option::is_none")]
    languages: Option<Vec<String>>,
    /// The call's one language, chosen when it starts (`call_language::for_new_call`); changed by hand straight in
    /// the file (`call_language::set`, under the call's lock) and kept from there
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    /// The AI profile this call's live notes and minutes use (`ai.rs`), pinned at the start
    #[serde(skip_serializing_if = "Option::is_none")]
    ai_profile: Option<String>,
    /// `memo`: the user alone, started by hand (no call; one track, the mic) — `calls::is_memo`.
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<&'static str>,
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

/// A new call's folder, created: `<stamp>-<slug of the title>`, its stamp the first second at or after `started` that no
/// folder in the calls folder or the trash starts with, so a stamp names one call for good (`calls::by_stamp`); the real
/// start stays in `meta.json`. Chosen and created under one lock: two recordings or imports at one instant get two.
pub(crate) fn call_dir(started: &DateTime<Local>, title: Option<&str>) -> Result<PathBuf> {
    let calls = paths::calls_dir();
    fs::create_dir_all(&calls).with_context(|| format!("creating {}", calls.display()))?;
    fs::create_dir_all(paths::state_dir())?;
    let lock = paths::state_dir().join("new-call.lock");
    let t = Instant::now();
    let _lock = loop {
        if let Some(l) = crate::app::FileLock::try_take(&lock).with_context(|| format!("opening {}", lock.display()))? {
            break l;
        }
        if t.elapsed() > Duration::from_secs(10) {
            anyhow::bail!("{} is held by another process", lock.display());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stamps = |d: PathBuf| fs::read_dir(d).into_iter().flatten().flatten().map(|e| crate::calls::stamp_of(&e.path()));
    let taken: std::collections::HashSet<String> = stamps(calls.clone()).chain(stamps(crate::calls::trash_dir())).collect();
    let slug = title.map(slugify).filter(|s| !s.is_empty()).map(|s| format!("-{s}")).unwrap_or_default();
    let mut at = *started;
    loop {
        let stamp = at.format("%Y-%m-%d_%H%M%S").to_string();
        if !taken.contains(&stamp) {
            let dir = calls.join(format!("{stamp}{slug}"));
            match fs::create_dir(&dir) {
                Ok(()) => return Ok(dir),
                // Made outside Echo since the folder was read: the next second.
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e).with_context(|| format!("creating {}", dir.display())),
            }
        }
        at += chrono::Duration::seconds(1);
    }
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
    /// None for a memo: only the user's voice.
    remote: Option<Track>,
    mic: Track,
    /// A memo (`start_memo`): no call behind it — it ends only when the user stops it.
    pub memo: bool,
}

impl Recording {
    /// The call app it records.
    pub fn app(&self) -> TargetApp {
        self.meta.app
    }

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
        let dir = call_dir(&started_at, title.as_deref())?;
        let language = crate::call_language::for_new_call(languages.as_deref(), &crate::config::Config::load().unwrap_or_default());
        let meta = Meta {
            title: title.clone(),
            app,
            platform: std::env::consts::OS,
            started_at,
            ended_at: None,
            stop_reason: None,
            languages: Some(vec![language.clone()]),
            language: Some(language),
            ai_profile,
            kind: None,
            tracks: vec![],
        };
        paths::write_atomic(&dir.join("meta.json"), &serde_json::to_vec_pretty(&meta)?)?;
        if let Some(m) = meeting {
            m.save(&dir)?;
        }
        // Both tracks share t0, so their timelines line up sample-for-sample.
        let t0 = Instant::now();
        let remote = Some(Track::start("remote", &dir.join("remote.wav"), audio::app_source(app), t0)?);
        let mic = Track::start("mic", &dir.join("mic.wav"), audio::mic_source(Some(app)), t0)?;
        // Transcribes the tracks as they are written (reads the WAVs, so it starts after them).
        #[cfg(feature = "transcribe")]
        let live = crate::live::start(&dir, meeting);
        #[cfg(not(feature = "transcribe"))]
        let live = None;
        Ok(Recording {
            live,
            dir,
            t0,
            title,
            started_at,
            meta,
            remote,
            mic,
            memo: false,
        })
    }

    /// A memo: the user alone, on the default microphone — no call, no other track. Titled "Memo"
    /// until its minutes name it; its threads are found from what was said (`memo_threads`).
    pub fn start_memo(app: TargetApp, languages: Option<Vec<String>>, ai_profile: Option<String>) -> Result<Self> {
        let started_at = Local::now();
        let title = Some("Memo".to_string());
        let dir = call_dir(&started_at, title.as_deref())?;
        let language = crate::call_language::for_new_call(languages.as_deref(), &crate::config::Config::load().unwrap_or_default());
        let meta = Meta {
            title: title.clone(),
            app,
            platform: std::env::consts::OS,
            started_at,
            ended_at: None,
            stop_reason: None,
            languages: Some(vec![language.clone()]),
            language: Some(language),
            ai_profile,
            kind: Some("memo"),
            tracks: vec![],
        };
        paths::write_atomic(&dir.join("meta.json"), &serde_json::to_vec_pretty(&meta)?)?;
        let t0 = Instant::now();
        // The default input, not the call app's: there is no call.
        let mic = Track::start("mic", &dir.join("mic.wav"), audio::mic_source(None), t0)?;
        #[cfg(feature = "transcribe")]
        let live = crate::live::start(&dir, None);
        #[cfg(not(feature = "transcribe"))]
        let live = None;
        Ok(Recording { live, dir, t0, title, started_at, meta, remote: None, mic, memo: true })
    }

    /// Switch the call's AI profile mid-call. The recording owns `meta.json` until it finishes
    /// (and rewrites it whole then), so the change goes through here.
    pub fn set_ai_profile(&mut self, id: &str) -> Result<()> {
        let _lock = crate::calls::lock_meta(&self.dir)?;
        keep_language(&self.dir, &mut self.meta);
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
        let remote = self.remote.as_mut().and_then(|r| r.poll()).map(|e| ("remote", e));
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
        if let Some(r) = remote.as_mut() {
            r.halt();
        }
        mic.halt();
        let mut errors = vec![];
        for track in remote.into_iter().chain([mic]) {
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
    keep_language(dir, meta);
    paths::write_atomic(&dir.join("meta.json"), &serde_json::to_vec_pretty(meta)?)?;
    Ok(())
}

/// The call's language as changed by hand meanwhile (`call_language::set`), into the recording's copy (under the
/// call's lock).
fn keep_language(dir: &std::path::Path, meta: &mut Meta) {
    let on_disk = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    if let Some(l) = on_disk.as_ref().and_then(|m| m["language"].as_str()).filter(|s| !s.is_empty()) {
        meta.language = Some(l.to_string());
        meta.languages = Some(vec![l.to_string()]);
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

    // `capture` leaves transcription to `echoctl transcribe`: the worker gives up, and is waited for so its model
    // is freed before the process exits (#141).
    let (dir, tracks, live) = rec.finish(if stop.load(Ordering::SeqCst) { "user" } else { "duration" })?;
    #[cfg(feature = "transcribe")]
    if let Some(l) = live {
        l.abandon();
    }
    #[cfg(not(feature = "transcribe"))]
    let _ = live;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calls::{by_stamp, stamp_of};

    fn at(s: &str) -> DateTime<Local> {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d_%H%M%S").unwrap().and_local_timezone(Local).unwrap()
    }

    /// A call folder as the recorder leaves it: made by the one helper, then its meta.
    fn new_call(started: &str, title: &str) -> PathBuf {
        let d = call_dir(&at(started), Some(title)).unwrap();
        fs::write(d.join("meta.json"), serde_json::json!({"title": title}).to_string()).unwrap();
        d
    }

    #[test]
    fn calls_started_in_one_second_get_the_next_free_stamps() {
        let (_env, _) = paths::testenv::folders("recorder-unique-stamps");
        let call = new_call("2026-11-18_100000", "Acme sync");
        let memo = new_call("2026-11-18_100000", "Memo");
        let third = new_call("2026-11-18_100000", "Acme sync");
        let stamps: Vec<_> = [&call, &memo, &third].map(|d| stamp_of(d)).into();
        assert_eq!(stamps, ["2026-11-18_100000", "2026-11-18_100001", "2026-11-18_100002"]);
        assert_eq!(third.file_name().unwrap(), "2026-11-18_100002-acme-sync", "no -2 after the slug");
        for (d, s) in [&call, &memo, &third].iter().zip(&stamps) {
            assert_eq!(by_stamp(s).as_ref(), Some(*d), "each opens by its stamp");
        }
    }

    #[test]
    fn a_stamp_held_by_a_folder_of_another_title_is_taken() {
        let (_env, _) = paths::testenv::folders("recorder-stamp-other-slug");
        // Another call's folder, not yet whole (an import writes meta.json last), and one in the trash.
        fs::create_dir_all(paths::calls_dir().join("2026-11-18_090000-globex-pilot")).unwrap();
        fs::create_dir_all(crate::calls::trash_dir().join("2026-11-18_090001-old-call.1")).unwrap();
        let d = new_call("2026-11-18_090000", "Initech planning");
        assert_eq!(d.file_name().unwrap(), "2026-11-18_090002-initech-planning");
    }

    #[test]
    fn two_threads_creating_at_one_instant_get_two_stamps() {
        let (_env, _) = paths::testenv::folders("recorder-stamp-race");
        let made: Vec<PathBuf> = std::thread::scope(|s| {
            let h: Vec<_> = (0..4).map(|_| s.spawn(|| call_dir(&at("2026-11-18_120000"), Some("Acme sync")).unwrap())).collect();
            h.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let stamps: std::collections::HashSet<String> = made.iter().map(|d| stamp_of(d)).collect();
        assert_eq!(stamps.len(), 4, "{made:?}");
    }

    #[test]
    fn the_stamp_survives_a_retitle() {
        let (_env, _) = paths::testenv::folders("recorder-stamp-retitle");
        let first = new_call("2026-11-18_140000", "Memo");
        let second = new_call("2026-11-18_140000", "Memo");
        let stamp = stamp_of(&second);
        crate::calls::update_meta(&second, |m| {
            m["title"] = serde_json::json!("Northwind kickoff");
            Ok(())
        })
        .unwrap();
        let renamed = crate::calls::rename_to_title(&second).unwrap();
        assert_eq!(renamed.file_name().unwrap(), "2026-11-18_140001-northwind-kickoff");
        assert_eq!((stamp_of(&renamed), by_stamp(&stamp)), (stamp.clone(), Some(renamed)));
        assert_eq!(by_stamp("2026-11-18_140000"), Some(first));
    }

    #[test]
    fn the_final_meta_keeps_the_language_the_live_transcription_wrote() {
        let (_env, _) = paths::testenv::folders("recorder-final-meta-language");
        let dir = paths::calls_dir().join("2026-11-09_140000-x");
        fs::create_dir_all(&dir).unwrap();
        let mut meta = Meta {
            title: Some("x".into()),
            app: TargetApp::Teams,
            platform: "macos",
            started_at: Local::now(),
            ended_at: None,
            stop_reason: None,
            languages: Some(vec!["en".into(), "uk".into()]),
            language: None,
            ai_profile: None,
            kind: None,
            tracks: vec![],
        };
        fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
        // The live worker decides the call's language mid-call (`calls::update_meta`).
        crate::calls::update_meta(&dir, |m| {
            m["language"] = serde_json::json!("uk");
            Ok(())
        })
        .unwrap();
        meta.stop_reason = Some("call_ended".into());
        write_final_meta(&dir, &mut meta).unwrap();
        let on_disk: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
        assert_eq!((on_disk["language"].as_str(), on_disk["stop_reason"].as_str()), (Some("uk"), Some("call_ended")));
    }
}
