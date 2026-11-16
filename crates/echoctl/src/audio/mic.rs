//! Microphone capture via cpal (CoreAudio on macOS, WASAPI on Windows).
//!
//! Which mic: the one the call app is recording from, when we can tell (macOS: Core Audio
//! process devices); otherwise the system default input. A headset user whose system default
//! is the laptop mic is thus recorded through the headset, like Teams hears them. The stream
//! reopens when the app switches mics, the default changes, the device errors/stalls, or its
//! rate changes.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream};

use super::{app_input_device, downmix, Chunk, Health, LiveStream, Source};
use crate::apps::TargetApp;

const STALL_AFTER: Duration = Duration::from_secs(2);

/// Set while the user is muted in the call app: the mic track then records silence (the
/// timeline stays intact), so nothing said off-mic is transcribed.
pub static MUTED: AtomicBool = AtomicBool::new(false);
const DEVICE_CHECK_EVERY: Duration = Duration::from_secs(1);
/// The app briefly shows no input device while switching: only a release longer than this counts.
const APP_DEVICE_GRACE: Duration = Duration::from_secs(2);

pub struct MicSource {
    pub app: Option<TargetApp>,
    /// The call app let go of its mic (the call ended, or it muted by closing it): the track records silence until
    /// the app holds a mic Echo can open again — never the room on another input (#140). On the source, not the
    /// stream, so a reopen meanwhile (a device error, a stall, a rate change) stays silent.
    pub released: Arc<AtomicBool>,
    /// The app has held a mic Echo opened at least once (or the fallback came): until then the track is silent too —
    /// a recording that starts before the app takes its mic doesn't record the room (#201).
    pub settled: Arc<AtomicBool>,
    pub started: Instant,
}

impl MicSource {
    /// A call's mic (`app`): silent until the app holds a mic Echo can open. A memo's (None): the default input.
    pub fn new(app: Option<TargetApp>) -> Self {
        MicSource { app, released: Arc::new(AtomicBool::new(app.is_some())), settled: Arc::new(AtomicBool::new(app.is_none())), started: Instant::now() }
    }
}

/// A call app that never reports a mic of its own (a virtual mic it uses, a listen-only join): after this long the
/// track takes the default input after all, rather than leave "Me" empty for the whole call.
const APP_MIC_FALLBACK: Duration = Duration::from_secs(10);

/// What the mic track does at a device check, from what the call app reports.
#[derive(Debug, PartialEq)]
enum Step {
    /// Nothing changes.
    Keep,
    /// Reopen on the app's mic (it switched, or Echo can open it now).
    FollowApp,
    /// The app let go of its mic: from now on silence.
    Release,
    /// Released, and the app took the very device open again: record it again.
    Resume,
    /// Not following an app: follow the default input.
    Default,
    /// The app reported no mic Echo can open since the recording began: take the default input after all.
    Fallback,
}

/// `app`: the app's mic, if it reports one — (is it the device open now, can Echo open it). `following`: the open
/// device is the app's. `released`: silent now. `missing_long`: the app has reported no mic for longer than the grace.
/// `waiting_long`: the app has held no mic Echo opened since the recording began, for longer than the fallback.
fn step(app: Option<(bool, bool)>, following: bool, released: bool, missing_long: bool, waiting_long: bool) -> Step {
    match app {
        Some((false, false)) | None if released && waiting_long => Step::Fallback,
        Some((true, _)) if released => Step::Resume,
        Some((true, _)) => Step::Keep,
        Some((false, true)) => Step::FollowApp,
        // A mic Echo can't open: silence rather than the default, once the app was followed.
        Some((false, false)) if released => Step::Keep,
        Some((false, false)) if following => Step::Release,
        Some((false, false)) => Step::Default,
        None if released => Step::Keep,
        None if following && missing_long => Step::Release,
        None if following => Step::Keep,
        None => Step::Default,
    }
}

fn device_name(d: &Device) -> String {
    d.description().map(|d| d.to_string()).unwrap_or_else(|_| "unknown input".into())
}

fn device_id(d: &Device) -> Option<String> {
    d.id().ok().map(|id| id.id().to_string())
}

/// The input cpal lists with this id, if any.
fn listed_input(uid: &str) -> Option<Device> {
    cpal::default_host().input_devices().ok()?.find(|d| device_id(d).as_deref() == Some(uid))
}

/// The device we should be recording from right now, and whether it came from the app.
fn desired_device(app: Option<TargetApp>) -> Result<(Device, bool)> {
    let host = cpal::default_host();
    if let Some((uid, _)) = app.and_then(app_input_device) {
        if let Some(d) = listed_input(&uid) {
            return Ok((d, true));
        }
    }
    let d = host.default_input_device().ok_or_else(|| anyhow!("no microphone available"))?;
    Ok((d, false))
}

/// Should a stream on the default input reopen on the app's mic `uid`? Only when cpal lists that
/// device: otherwise the reopen lands on the default again and the next check asks again — a
/// reopen every second. Such an id is remembered in `unmatched` (`listed` is asked again at each
/// check, so a device that appears later is still followed).
fn follow_app_mic(uid: &str, open_id: Option<&str>, listed: impl FnOnce() -> bool, unmatched: &mut Option<String>) -> bool {
    if open_id == Some(uid) {
        *unmatched = None;
        return false;
    }
    if listed() {
        *unmatched = None;
        return true;
    }
    *unmatched = Some(uid.to_string());
    false
}

struct MicStream {
    _stream: Stream,
    /// The device open, asked for its rate each check (no enumeration of every input).
    dev: Device,
    app: Option<TargetApp>,
    device: String,
    device_id: Option<String>,
    from_app: bool,
    rate: u32,
    lost: Arc<AtomicBool>,
    callbacks: Arc<AtomicU64>,
    seen: (u64, Instant),
    last_device_check: Instant,
    app_device_missing_since: Option<Instant>,
    /// `MicSource::released`, `settled` and `started`, shared.
    released: Arc<AtomicBool>,
    settled: Arc<AtomicBool>,
    started: Instant,
    /// The app's mic when cpal doesn't list it (we stay on the default meanwhile).
    unmatched_app_mic: Option<String>,
}

impl Source for MicSource {
    fn open(&mut self, epoch: u32, tx: Sender<Chunk>) -> Result<Box<dyn LiveStream>> {
        let (device, from_app) = desired_device(self.app)?;
        let name = device_name(&device);
        let id = device_id(&device);
        let supported = device.default_input_config().context("querying mic config")?;
        let channels = supported.channels() as usize;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let rate = config.sample_rate;

        let lost = Arc::new(AtomicBool::new(false));
        let callbacks = Arc::new(AtomicU64::new(0));
        if from_app {
            self.released.store(false, Ordering::SeqCst);
            self.settled.store(true, Ordering::SeqCst);
        }
        let released = self.released.clone();
        let (l, c, r) = (lost.clone(), callbacks.clone(), released.clone());
        let err = move |e: cpal::Error| {
            // An overrun is a glitch (samples dropped; the writer pads the gap) and the stream
            // goes on — WASAPI flags one whenever a packet is late. A dead device shows up as a
            // real error, or as a stall.
            if matches!(e.kind(), cpal::ErrorKind::Xrun) {
                return;
            }
            eprintln!("  mic stream error: {e}");
            l.store(true, Ordering::SeqCst);
        };
        let send = move |samples: Vec<f32>| {
            c.fetch_add(1, Ordering::Relaxed);
            let samples = if MUTED.load(Ordering::Relaxed) || r.load(Ordering::Relaxed) { vec![0.0; samples.len()] } else { samples };
            let _ = tx.send(Chunk { epoch, rate, samples });
        };
        let stream = match format {
            SampleFormat::F32 => device.build_input_stream(
                config,
                move |data: &[f32], _: &_| send(downmix(data, channels)),
                err,
                None,
            ),
            SampleFormat::I16 => device.build_input_stream(
                config,
                move |data: &[i16], _: &_| {
                    let f: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                    send(downmix(&f, channels))
                },
                err,
                None,
            ),
            other => return Err(anyhow!("unsupported mic sample format {other:?}")),
        }
        .context("opening microphone stream")?;
        stream.play().context("starting microphone stream")?;
        Ok(Box::new(MicStream {
            _stream: stream,
            dev: device.clone(),
            app: self.app,
            device: name,
            device_id: id,
            from_app,
            rate,
            lost,
            callbacks,
            seen: (0, Instant::now()),
            last_device_check: Instant::now(),
            app_device_missing_since: None,
            released,
            settled: self.settled.clone(),
            started: self.started,
            unmatched_app_mic: None,
        }))
    }
}

impl MicStream {
    /// Has the device we *should* use changed? Returns a restart reason.
    fn device_change(&mut self) -> Option<String> {
        let app_dev = self.app.and_then(app_input_device);
        let released = self.released.load(Ordering::SeqCst);
        if app_dev.is_some() {
            self.app_device_missing_since = None;
        }
        let missing_long = app_dev.is_none() && self.app_device_missing_since.get_or_insert_with(Instant::now).elapsed() >= APP_DEVICE_GRACE;
        // The device open is listed by definition; another one is looked for (an enumeration) only when the app moved.
        let app = app_dev.as_ref().map(|(uid, _)| {
            let open = Some(uid) == self.device_id.as_ref();
            (open, open || listed_input(uid).is_some())
        });
        let name = app_dev.as_ref().map(|(_, n)| n.as_str()).unwrap_or("");
        let who = self.app.map(|a| format!("{a:?}")).unwrap_or_default();
        // The device open is the app's mic (taken back after a reopen onto it, or reported late): from now on this
        // stream follows the app, so its next release is silence too, never the default.
        if matches!(app, Some((true, _))) {
            self.from_app = true;
        }
        let waiting_long = !self.settled.load(Ordering::SeqCst) && self.started.elapsed() >= APP_MIC_FALLBACK;
        match step(app, self.from_app, released, missing_long, waiting_long) {
            Step::Keep => None,
            Step::FollowApp => {
                self.released.store(false, Ordering::SeqCst);
                self.settled.store(true, Ordering::SeqCst);
                Some(if self.from_app || released { format!("{who} switched mic to \"{name}\"") } else { format!("following {who}'s mic \"{name}\"") })
            }
            Step::Release => {
                self.released.store(true, Ordering::SeqCst);
                eprintln!("  {who} released its mic: the mic track records silence until it takes one again");
                None
            }
            Step::Fallback => {
                self.released.store(false, Ordering::SeqCst);
                self.settled.store(true, Ordering::SeqCst);
                eprintln!("  {who} holds no mic Echo can open after {} s: recording the default input \"{}\"", APP_MIC_FALLBACK.as_secs(), self.device);
                None
            }
            Step::Resume => {
                self.released.store(false, Ordering::SeqCst);
                self.settled.store(true, Ordering::SeqCst);
                eprintln!("  {who} took its mic again: recording it");
                None
            }
            Step::Default => {
                if let Some((uid, name)) = &app_dev {
                    let was = self.unmatched_app_mic.clone();
                    follow_app_mic(uid, self.device_id.as_deref(), || false, &mut self.unmatched_app_mic);
                    if self.unmatched_app_mic != was {
                        eprintln!("  {who}'s mic \"{name}\" isn't in the input list; staying on \"{}\"", self.device);
                    }
                }
                self.default_change()
            }
        }
    }

    /// Using the default: follow changes to it.
    fn default_change(&self) -> Option<String> {
        let d = cpal::default_host().default_input_device()?;
        (device_id(&d) != self.device_id).then(|| format!("default mic changed to \"{}\"", device_name(&d)))
    }
}

impl LiveStream for MicStream {
    fn label(&self) -> String {
        let via = if self.from_app { " (the mic the call app uses)" }
            else if self.released.load(Ordering::SeqCst) { " (system default, silent until the call app holds a mic)" }
            else { " (system default)" };
        format!("microphone \"{}\" @ {} Hz{via}", self.device, self.rate)
    }

    fn health(&mut self) -> Health {
        if self.lost.load(Ordering::SeqCst) {
            return Health::Restart(format!("\"{}\" reported an error (disconnected?)", self.device));
        }
        let n = self.callbacks.load(Ordering::Relaxed);
        if n != self.seen.0 {
            self.seen = (n, Instant::now());
        } else if self.seen.1.elapsed() > STALL_AFTER {
            return Health::Restart(format!("\"{}\" stopped delivering audio", self.device));
        }
        if self.last_device_check.elapsed() >= DEVICE_CHECK_EVERY {
            self.last_device_check = Instant::now();
            if let Some(reason) = self.device_change() {
                return Health::Reopen(reason);
            }
            if let Ok(cfg) = self.dev.default_input_config() {
                if cfg.sample_rate() != self.rate {
                    return Health::Reopen(format!(
                        "\"{}\" changed rate {} → {} Hz",
                        self.device,
                        self.rate,
                        cfg.sample_rate()
                    ));
                }
            }
        }
        Health::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn after_the_app_lets_go_of_its_mic_the_track_stays_silent_until_it_takes_one_echo_can_open() {
        // (app's mic: (open now, listed)), following, released, missing for longer than the grace
        assert_eq!(step(None, true, false, false, false), Step::Keep, "a blip shorter than the grace");
        assert_eq!(step(None, true, false, true, false), Step::Release, "released after the grace");
        // A reopen while released (device error, stall, rate change) lands on the default input, not followed:
        assert_eq!(step(None, false, true, true, false), Step::Keep, "still silent, never the default (#140)");
        assert_eq!(step(Some((false, false)), false, true, false, false), Step::Keep, "a mic Echo can't open: still silent");
        assert_eq!(step(Some((false, false)), true, false, false, false), Step::Release, "following, the app moved to a mic Echo can't open");
        assert_eq!(step(Some((true, true)), true, true, false, false), Step::Resume, "the same device again");
        // Released, reopened onto the default, the app takes that very device: the stream now follows the app
        // (device_change marks it), so the next release is silence again, not the default (#140).
        assert_eq!(step(None, true, false, true, false), Step::Release, "released again: silence");
        assert_eq!(step(None, false, false, true, false), Step::Default, "what an unmarked stream would do — the bug");
        assert_eq!(step(Some((false, true)), false, true, false, false), Step::FollowApp, "a device Echo can open");
        assert_eq!(step(Some((false, true)), true, false, false, false), Step::FollowApp, "the app switched mic");
        // Settled on the default (after the fallback, or a memo): follow the default input.
        assert_eq!(step(None, false, false, true, false), Step::Default);
        assert_eq!(step(Some((false, false)), false, false, false, false), Step::Default);
        assert_eq!(step(Some((true, true)), true, false, false, false), Step::Keep);
        // Before the app holds a mic (#201): silent, then the default input after the fallback, never before.
        assert_eq!(step(None, false, true, false, false), Step::Keep, "waiting: silent");
        assert_eq!(step(Some((false, false)), false, true, false, false), Step::Keep, "a mic Echo can't open yet: silent");
        assert_eq!(step(Some((false, true)), false, true, false, false), Step::FollowApp, "the app's mic: follow it");
        assert_eq!(step(None, false, true, true, true), Step::Fallback, "no mic of its own after the fallback");
        assert_eq!(step(Some((false, false)), false, true, false, true), Step::Fallback);
    }

    #[test]
    fn an_unlisted_app_mic_is_not_followed_until_it_appears() {
        let mut unmatched = None;
        // Teams reports a mic cpal doesn't list: stay on the default, check after check.
        for _ in 0..5 {
            assert!(!follow_app_mic("missing-input", Some("default-mic"), || false, &mut unmatched));
        }
        assert_eq!(unmatched.as_deref(), Some("missing-input"));
        // It shows up in the list: follow it.
        assert!(follow_app_mic("missing-input", Some("default-mic"), || true, &mut unmatched));
        assert_eq!(unmatched, None);
        // Already on it: nothing to do, and the list isn't asked.
        assert!(!follow_app_mic("headset", Some("headset"), || panic!("not asked"), &mut unmatched));
    }
}
