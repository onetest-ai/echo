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
/// The app briefly shows no input device while switching; don't fall back to default on a blip.
const APP_DEVICE_GRACE: Duration = Duration::from_secs(2);

pub struct MicSource {
    pub app: Option<TargetApp>,
}

fn device_name(d: &Device) -> String {
    d.description().map(|d| d.to_string()).unwrap_or_else(|_| "unknown input".into())
}

fn device_id(d: &Device) -> Option<String> {
    d.id().ok().map(|id| id.id().to_string())
}

/// The device we should be recording from right now, and whether it came from the app.
fn desired_device(app: Option<TargetApp>) -> Result<(Device, bool)> {
    let host = cpal::default_host();
    if let Some((uid, _)) = app.and_then(app_input_device) {
        if let Ok(mut inputs) = host.input_devices() {
            if let Some(d) = inputs.find(|d| device_id(d).as_deref() == Some(uid.as_str())) {
                return Ok((d, true));
            }
        }
    }
    let d = host.default_input_device().ok_or_else(|| anyhow!("no microphone available"))?;
    Ok((d, false))
}

struct MicStream {
    _stream: Stream,
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
        let (l, c) = (lost.clone(), callbacks.clone());
        let err = move |e| {
            eprintln!("  mic stream error: {e}");
            l.store(true, Ordering::SeqCst);
        };
        let send = move |samples: Vec<f32>| {
            c.fetch_add(1, Ordering::Relaxed);
            let samples = if MUTED.load(Ordering::Relaxed) { vec![0.0; samples.len()] } else { samples };
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
        }))
    }
}

impl MicStream {
    /// Has the device we *should* use changed? Returns a restart reason.
    fn device_change(&mut self) -> Option<String> {
        let app_dev = self.app.and_then(app_input_device);
        match (&app_dev, self.from_app) {
            // Following the app, and it switched to another mic.
            (Some((uid, name)), true) if Some(uid) != self.device_id.as_ref() => {
                Some(format!("{:?} switched mic to \"{name}\"", self.app.unwrap()))
            }
            // Using the default, but the app now has a mic of its own.
            (Some((uid, name)), false) if Some(uid) != self.device_id.as_ref() => {
                Some(format!("following {:?}'s mic \"{name}\"", self.app.unwrap()))
            }
            (Some(_), _) => {
                self.app_device_missing_since = None;
                None
            }
            // Was following the app, which no longer reports a mic: fall back after a grace.
            (None, true) => {
                let since = *self.app_device_missing_since.get_or_insert_with(Instant::now);
                (since.elapsed() >= APP_DEVICE_GRACE)
                    .then(|| format!("{:?} released its mic; using the default input", self.app.unwrap()))
            }
            // Using the default: follow changes to it.
            (None, false) => {
                let d = cpal::default_host().default_input_device()?;
                (device_id(&d) != self.device_id).then(|| format!("default mic changed to \"{}\"", device_name(&d)))
            }
        }
    }
}

impl LiveStream for MicStream {
    fn label(&self) -> String {
        let via = if self.from_app { " (the mic the call app uses)" } else { " (system default)" };
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
                return Health::Restart(reason);
            }
            let host = cpal::default_host();
            let current = host
                .input_devices()
                .ok()
                .and_then(|mut it| it.find(|d| device_id(d) == self.device_id));
            if let Some(cfg) = current.and_then(|d| d.default_input_config().ok()) {
                if cfg.sample_rate() != self.rate {
                    return Health::Restart(format!(
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
