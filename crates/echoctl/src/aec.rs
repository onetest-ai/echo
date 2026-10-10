//! Acoustic echo cancellation of the mic track, offline, before transcription.
//!
//! When the user is on speakers, the mic re-records the remote participants. The two tracks
//! share one clock, so the remote track is an exact echo reference: WebRTC's AEC3 (pure-Rust
//! port) subtracts it from a copy of the mic audio, which is what gets transcribed. The raw
//! recording is never changed. Headphones (no echo) are detected and left alone; the
//! transcript-level echo filter (`remove_echo`) still runs afterwards as a safety net.

use aec3::nodes::audio::AudioFormat;
use aec3::pipelines::linear;
use anyhow::{Context, Result};

const RATE: u32 = 16_000;
const FRAME: usize = (RATE / 100) as usize; // 10 ms
/// Envelope frames for echo detection (30 ms) and the delays searched (speaker → mic path).
const ENV: usize = 480;
const MAX_LAG_FRAMES: usize = 14; // ~420 ms
/// Correlation of the loudness envelopes above which the mic is taken to carry echo.
const ECHO_CORRELATION: f32 = 0.35;

fn envelope(x: &[f32]) -> Vec<f32> {
    x.chunks(ENV).map(|f| (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt()).collect()
}

fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 10 {
        return 0.0;
    }
    let (a, b) = (&a[..n], &b[..n]);
    let (ma, mb) = (a.iter().sum::<f32>() / n as f32, b.iter().sum::<f32>() / n as f32);
    let (mut cov, mut va, mut vb) = (0f32, 0f32, 0f32);
    for i in 0..n {
        let (x, y) = (a[i] - ma, b[i] - mb);
        cov += x * y;
        va += x * x;
        vb += y * y;
    }
    if va <= 0.0 || vb <= 0.0 {
        0.0
    } else {
        cov / (va.sqrt() * vb.sqrt())
    }
}

/// Does the mic carry the remote track's echo? Returns the delay (ms) if so.
pub fn echo_delay_ms(mic: &[f32], remote: &[f32]) -> Option<i32> {
    let (em, er) = (envelope(mic), envelope(remote));
    // Only where the remote side is talking does an echo show.
    let (best_lag, best) = (0..=MAX_LAG_FRAMES)
        .map(|lag| (lag, correlation(&em[lag.min(em.len())..], &er)))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    (best >= ECHO_CORRELATION).then_some((best_lag * ENV * 1000 / RATE as usize) as i32)
}

/// Echo-cancelled copy of `mic` (16 kHz mono), or None when there is no echo to remove.
pub fn cancel(mic: &[f32], remote: &[f32]) -> Result<Option<(Vec<f32>, i32)>> {
    let Some(delay) = echo_delay_ms(mic, remote) else { return Ok(None) };
    let format = AudioFormat::ten_ms(RATE, 1);
    // Echo removal only: noise suppression and gain control would change the levels the VAD
    // and Whisper see.
    let mut p = linear::builder(format, format)
        .initial_delay_ms(delay)
        .enable_noise_suppression(false)
        .enable_gain_controller2(false)
        .build()
        .context("building the echo canceller")?;
    let mut out = Vec::with_capacity(mic.len());
    let frame = |x: &[f32], at: usize| -> Vec<f32> {
        let mut f = vec![0.0; FRAME];
        if at < x.len() {
            let n = (x.len() - at).min(FRAME);
            f[..n].copy_from_slice(&x[at..at + n]);
        }
        f
    };
    let mut at = 0;
    while at < mic.len() {
        p.handle_render_frame(&frame(remote, at)).context("echo reference")?;
        let valid = (mic.len() - at).min(FRAME);
        let mut processed = vec![0.0f32; FRAME];
        if p.process_capture_frame(&frame(mic, at), &mut processed).context("echo cancellation")? {
            out.extend_from_slice(&processed[..valid]);
        } else {
            out.extend_from_slice(&mic[at..at + valid]);
        }
        at += valid;
    }
    Ok(Some((out, delay)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic "speech": noise in random 0.3–1.5 s bursts with random pauses.
    fn bursts(seed: u32, len: usize) -> Vec<f32> {
        let mut x = seed;
        let mut next = move || {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            x
        };
        let mut out = Vec::with_capacity(len);
        let mut talking = false;
        while out.len() < len {
            let dur = 4800 + (next() % 19_200) as usize;
            for _ in 0..dur.min(len - out.len()) {
                let v = next();
                out.push(if talking { ((v >> 9) as f32 / (1u32 << 23) as f32 - 0.5) * 0.4 } else { 0.0 });
            }
            talking = !talking;
        }
        out
    }

    fn energy(x: &[f32]) -> f32 {
        x.iter().map(|s| s * s).sum::<f32>()
    }

    #[test]
    fn headphones_mean_no_echo() {
        // Independent speakers: the mic hears only the user.
        let n = RATE as usize * 20;
        assert_eq!(echo_delay_ms(&bursts(2, n), &bursts(1, n)), None);
    }

    #[test]
    fn echo_is_found_and_reduced() {
        let n = RATE as usize * 10;
        let remote = bursts(3, n);
        let delay = 1600; // 100 ms speaker → mic
        let mic: Vec<f32> = (0..n).map(|i| if i >= delay { remote[i - delay] * 0.5 } else { 0.0 }).collect();
        let found = echo_delay_ms(&mic, &remote).expect("echo detected");
        assert!((60..=150).contains(&found), "delay {found} ms");
        let (clean, _) = cancel(&mic, &remote).unwrap().unwrap();
        // After convergence (skip the first 3 s) most of the echo is gone.
        let tail = RATE as usize * 3;
        assert!(energy(&clean[tail..]) < energy(&mic[tail..]) * 0.5, "echo not reduced");
    }
}
