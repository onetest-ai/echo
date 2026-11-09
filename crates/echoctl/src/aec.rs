//! Acoustic echo cancellation of the mic track, offline, before transcription.
//!
//! When the user is on speakers, the mic re-records the remote participants. The two tracks
//! share one clock, so the remote track is an exact echo reference: WebRTC's AEC3 (pure-Rust
//! port) subtracts it from a copy of the mic audio, which is what gets transcribed. The raw
//! recording is never changed. Headphones (no echo) are detected and left alone; the
//! transcript-level echo filter (`remove_echo`) still runs afterwards as a safety net.
//!
//! During a call the same loudness envelopes find the echo (`find`: its delay, gain and correlation) once the other
//! side has talked for a while, and `Echo::keep` tells which stretches of a mic utterance it explains, so the live
//! transcription (`live.rs`) skips them instead of cancelling and decoding the whole mic again at the end.

use aec3::nodes::audio::AudioFormat;
use aec3::pipelines::linear;
use anyhow::{Context, Result};

const RATE: u32 = 16_000;
const FRAME: usize = (RATE / 100) as usize; // 10 ms
/// Envelope frames for echo detection (30 ms) and the delays searched (speaker → mic path).
pub(crate) const ENV: usize = 480;
const MAX_LAG_FRAMES: usize = 14; // ~420 ms
/// Correlation of the loudness envelopes above which the mic is taken to carry echo.
const ECHO_CORRELATION: f32 = 0.35;
/// The remote side talks in a frame at least this loud against its loud frames (−20 dB of the 99th percentile).
const TALK_FLOOR: f32 = 0.1;
/// Least gain taken as an echo (−26 dB): fainter, cancelling it changes nothing worth a decode, and a chance
/// correlation on headphones comes with a gain about this small.
const MIN_GAIN: f32 = 0.05;
/// Least remote talk (in frames) to decide on: 5 s.
const MIN_TALK: usize = 5000 / 30;

/// Loudness (RMS) of `x` in 30 ms frames.
pub(crate) fn envelope(x: &[f32]) -> Vec<f32> {
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

fn percentile(x: &[f32], p: f32) -> f32 {
    let mut v = x.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    v.get(((v.len() as f32 - 1.0) * p) as usize).copied().unwrap_or(0.0)
}

/// The frames of the remote envelope in which the other side talks.
fn talking(er: &[f32]) -> Vec<usize> {
    let sound: Vec<f32> = er.iter().copied().filter(|&v| v > 1e-4).collect();
    let floor = (percentile(&sound, 0.99) * TALK_FLOOR).max(1e-4);
    (0..er.len()).filter(|&i| er[i] >= floor).collect()
}

/// The echo of the remote track in the mic track: how late (in envelope frames), how loud, how sure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Echo {
    pub lag: usize,
    /// Mic loudness per unit of remote loudness (the median over the frames where the remote talks).
    pub gain: f32,
    /// Correlation of the envelopes there.
    pub r: f32,
}

impl Echo {
    pub fn delay_ms(&self) -> i32 {
        (self.lag * ENV * 1000 / RATE as usize) as i32
    }

    /// The same echo as `other`: its delay within a frame, its gain within `TOLERANCE` (what `keep` allows).
    pub fn same_as(&self, other: &Echo) -> bool {
        self.lag.abs_diff(other.lag) <= 1 && (1.0 / TOLERANCE..=TOLERANCE).contains(&(self.gain / other.gain))
    }
}

/// Does the mic carry the remote track's echo? From the two envelopes, with at least `min_talk` frames of the other
/// side talking. Only where the remote talks does an echo show: over the whole call the two sides take turns, and
/// their envelopes are anti-correlated, which hides the echo (#238).
pub(crate) fn find(em: &[f32], er: &[f32], min_talk: usize) -> Option<Echo> {
    let talk = talking(er);
    if talk.len() < min_talk.max(MIN_TALK) {
        return None;
    }
    let first = best_lag(em, er, &talk)?;
    // A second look without the frames the user's own voice makes louder than the echo could be (talking over the
    // other side): they drown the echo's correlation (0.96 → 0.5 with four such lines in two minutes).
    let quiet: Vec<usize> = talk
        .iter()
        .copied()
        .filter(|&i| i + first.lag < em.len() && em[i + first.lag] <= MARGIN * first.gain * er[i.saturating_sub(3)..(i + 2).min(er.len())].iter().copied().fold(0.0, f32::max))
        .collect();
    let e = if quiet.len() >= MIN_TALK { best_lag(em, er, &quiet)? } else { first };
    (e.r >= ECHO_CORRELATION && e.gain >= MIN_GAIN).then_some(e)
}

/// The lag at which the mic's envelope follows the remote's best over the remote's `frames`, how closely, and the
/// median gain there.
fn best_lag(em: &[f32], er: &[f32], frames: &[usize]) -> Option<Echo> {
    let pairs = |lag: usize| -> (Vec<f32>, Vec<f32>) { frames.iter().filter(|&&i| i + lag < em.len()).map(|&i| (em[i + lag], er[i])).unzip() };
    let (lag, r) = (0..=MAX_LAG_FRAMES)
        .map(|lag| {
            let (m, r) = pairs(lag);
            (lag, correlation(&m, &r))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let (m, rr) = pairs(lag);
    let ratios: Vec<f32> = m.iter().zip(&rr).map(|(m, r)| m / r).collect();
    Some(Echo { lag, gain: percentile(&ratios, 0.5), r })
}

/// A mic frame up to this many times the echo's loudness (gain × the remote's) is the echo alone.
const MARGIN: f32 = 2.0;
/// The user's own voice is kept with this many frames around it (240 ms), and two stretches of it this close (800 ms)
/// as one.
const PAD: usize = 8;
const BRIDGE: usize = 27;
/// A stretch between them is dropped as echo only with this many voiced frames (300 ms), its envelope following the
/// remote's this closely, and the mic within `TOLERANCE` (× either way, about 1.6 dB) of gain × the remote's in nine
/// frames in ten.
const MIN_VOICED: usize = 10;
const ECHO_STRETCH_R: f32 = 0.5;
const TOLERANCE: f32 = 1.2;
/// This many voiced frames in a row (90 ms) louder than the echo's band are the user's voice, even under `MARGIN`: a
/// word said at the echo's level. Only isolated frames go to the `is_echo` allowance.
const RUN: usize = 3;
/// The speech floor is taken over the last 60 s of the mic, as `transcribe::utterances` takes it over its buffer.
const FLOOR_WINDOW: usize = 2000;

/// The loudness above which a mic frame is speech: 12 dB over the noise floor (20th percentile), within −55…−35 dBFS
/// (`transcribe::utterances`' threshold).
fn speech_floor(em: &[f32]) -> f32 {
    let db = (20.0 * percentile(em, 0.2).max(1e-9).log10() + 12.0).clamp(-55.0, -35.0);
    10f32.powf(db / 20.0)
}

impl Echo {
    /// The parts of the mic's frames `from..to` (an utterance) to transcribe, as frame ranges: all of it but the
    /// stretches the remote's echo explains along their whole length. A frame is the user's own when the mic is
    /// louder there than `MARGIN` × the echo (the remote's loudest at the delay, ± a frame or so for the room's
    /// tail); those are kept with `PAD` around them. A stretch between them is dropped only when it is the echo
    /// (`is_echo`); a short or silent one is kept. When the envelopes don't reach `to` yet: all of it (in doubt,
    /// transcribe).
    ///
    /// Loudness can't tell apart a sound the echo hides: words 6 dB or more below the echo, or very short sounds (under
    /// about 150 ms) at its level, can be dropped with it.
    pub fn keep(&self, em: &[f32], er: &[f32], from: usize, to: usize) -> Vec<(usize, usize)> {
        if from >= to || to > em.len() || er.len() + self.lag <= to {
            return vec![(from, to)];
        }
        let floor = speech_floor(&em[from.saturating_sub(FLOOR_WINDOW)..to]);
        let heard = |i: usize| -> f32 {
            let c = i as isize - self.lag as isize;
            (c - 3..=c + 1).filter(|&j| j >= 0 && (j as usize) < er.len()).map(|j| er[j as usize]).fold(0.0, f32::max)
        };
        // Over the echo's band (`is_echo`'s) for `RUN` frames in a row: the user's voice too.
        let over: Vec<bool> = (from..to).map(|i| em[i] > floor / 2.0 && em[i] > self.band(er, i).1).collect();
        let in_run = |k: usize| {
            let (mut a, mut b) = (k, k);
            while a > 0 && over[a - 1] {
                a -= 1;
            }
            while b + 1 < over.len() && over[b + 1] {
                b += 1;
            }
            over[k] && b + 1 - a >= RUN
        };
        let mut own: Vec<(usize, usize)> = vec![];
        for i in (from..to).filter(|&i| (em[i] > floor && em[i] > MARGIN * self.gain * heard(i)) || in_run(i - from)) {
            let (a, b) = (i.saturating_sub(PAD).max(from), (i + 1 + PAD).min(to));
            match own.last_mut() {
                Some(last) if a <= last.1 + BRIDGE => last.1 = b,
                _ => own.push((a, b)),
            }
        }
        let mut out: Vec<(usize, usize)> = vec![];
        let mut add = |a: usize, b: usize| match out.last_mut() {
            Some(last) if last.1 == a => last.1 = b,
            _ => out.push((a, b)),
        };
        let mut at = from;
        for (a, b) in own.into_iter().chain([(to, to)]) {
            if at < a && !self.is_echo(em, er, at, a, floor) {
                add(at, a);
            }
            if a < b {
                add(a, b);
            }
            at = b;
        }
        out
    }

    /// The mic's frames `a..b` are the remote's echo: enough speech, its envelope following the remote's at the
    /// delay, and its loudness gain × the remote's frame by frame (`TOLERANCE`, nine frames in ten). A voice of the
    /// user's own as quiet as the echo can follow the remote's envelope by chance over a short stretch, but not its
    /// loudness.
    fn is_echo(&self, em: &[f32], er: &[f32], a: usize, b: usize, floor: f32) -> bool {
        let at = |i: usize| self.at(er, i);
        let (m, r): (Vec<f32>, Vec<f32>) = (a..b).filter(|&i| em[i] > floor).map(|i| (em[i], at(i))).unzip();
        if m.len() < MIN_VOICED || correlation(&m, &r) < ECHO_STRETCH_R {
            return false;
        }
        let band = |i: usize| self.band(er, i);
        let due: Vec<usize> = (a..b).filter(|&i| em[i] > floor || band(i).1 > floor * TOLERANCE).collect();
        let fit = due.iter().filter(|&&i| (band(i).0..=band(i).1).contains(&em[i])).count();
        fit * 10 >= due.len() * 9
    }

    /// The remote's loudness that mic frame `i` hears as echo (0 before the delay).
    fn at(&self, er: &[f32], i: usize) -> f32 {
        i.checked_sub(self.lag).and_then(|j| er.get(j)).copied().unwrap_or(0.0)
    }

    /// How loud the echo in mic frame `i` can be, (least, most): the delay falls between two frames, so between the
    /// remote's frames around it × gain, `TOLERANCE` either way.
    fn band(&self, er: &[f32], i: usize) -> (f32, f32) {
        let r = [self.at(er, i.saturating_sub(1)), self.at(er, i), self.at(er, i + 1)];
        (self.gain * r.iter().copied().fold(f32::MAX, f32::min) / TOLERANCE, self.gain * r.iter().copied().fold(0.0, f32::max) * TOLERANCE)
    }
}

/// [`find`] on the two tracks (16 kHz mono).
pub(crate) fn detect(mic: &[f32], remote: &[f32]) -> Option<Echo> {
    find(&envelope(mic), &envelope(remote), MIN_TALK)
}

/// Echo-cancelled copy of `mic` (16 kHz mono), or None when there is no echo to remove.
pub fn cancel(mic: &[f32], remote: &[f32]) -> Result<Option<(Vec<f32>, i32)>> {
    let Some(delay) = detect(mic, remote).map(|e| e.delay_ms()) else { return Ok(None) };
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

/// Speech-like test signals: turns of syllables, and their echo.
#[cfg(test)]
pub(crate) mod signals {
    /// Speech-like turns: noise in syllables of 100–300 ms at varying loudness, a turn 1–3 s long.
    pub(crate) fn syllables(next: &mut impl FnMut() -> u32, out: &mut Vec<f32>, len: usize) {
        let end = out.len() + len;
        while out.len() < end {
            let (dur, amp) = (1600 + (next() % 3200) as usize, 0.05 + (next() % 1000) as f32 / 1000.0 * 0.35);
            for _ in 0..dur.min(end - out.len()) {
                out.push(((next() >> 9) as f32 / (1u32 << 23) as f32 - 0.5) * amp);
            }
        }
    }

    /// A conversation that takes turns: (the user's voice, the remote track). The user never talks over the other side.
    pub(crate) fn turn_taking(seed: u32, len: usize) -> (Vec<f32>, Vec<f32>) {
        let mut x = seed;
        let mut next = move || {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            x
        };
        let (mut user, mut remote) = (vec![], vec![]);
        let mut remote_turn = true;
        while remote.len() < len {
            let (turn, gap) = (16_000 + (next() % 32_000) as usize, 8000 + (next() % 8000) as usize);
            let (talks, quiet) = if remote_turn { (&mut remote, &mut user) } else { (&mut user, &mut remote) };
            syllables(&mut next, talks, turn);
            quiet.resize(talks.len(), 0.0);
            remote.resize(remote.len() + gap, 0.0);
            user.resize(remote.len(), 0.0);
            remote_turn = !remote_turn;
        }
        user.truncate(len);
        remote.truncate(len);
        (user, remote)
    }

    /// `mic` with `remote` added `delay` samples late at `gain`, as laptop speakers do.
    pub(crate) fn with_echo(mic: &[f32], remote: &[f32], gain: f32, delay: usize) -> Vec<f32> {
        mic.iter().enumerate().map(|(i, m)| m + if i >= delay { remote[i - delay] * gain } else { 0.0 }).collect()
    }

    /// The user talks over the other side: speech added to `user` over the second half of every third remote turn.
    pub(crate) fn talk_over(user: &mut [f32], remote: &[f32], seed: u32) {
        let mut x = seed;
        let mut next = move || {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            x
        };
        let (mut turns, mut i) = (vec![], 0);
        while i < remote.len() {
            if remote[i] == 0.0 {
                i += 1;
                continue;
            }
            let start = i;
            while i < remote.len() && remote[i..(i + 800).min(remote.len())].iter().any(|&v| v != 0.0) {
                i += 800;
            }
            turns.push((start, i.min(remote.len())));
        }
        for &(s, e) in turns.iter().skip(1).step_by(3) {
            let mut v = vec![];
            syllables(&mut next, &mut v, e - (s + e) / 2);
            for (k, x) in v.into_iter().enumerate() {
                user[(s + e) / 2 + k] += x;
            }
        }
    }

    /// A room's noise floor (about −50 dBFS) under `x`.
    pub(crate) fn with_noise(x: &[f32], seed: u32) -> Vec<f32> {
        let mut r = seed;
        x.iter()
            .map(|s| {
                r = r.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                s + ((r >> 9) as f32 / (1u32 << 23) as f32 - 0.5) * 0.01
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::signals::*;
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
        assert_eq!(detect(&bursts(2, n), &bursts(1, n)), None);
    }

    #[test]
    fn an_echo_is_found_in_a_conversation_that_takes_turns() {
        // Over the whole call the sides alternate (anti-correlated), which hid a 0.35× echo (#238); where the
        // remote talks, the mic follows it.
        let n = RATE as usize * 90;
        let (user, remote) = turn_taking(7, n);
        assert_eq!(detect(&user, &remote), None, "no echo on headphones");
        let e = detect(&with_echo(&user, &remote, 0.35, 640), &remote).expect("a 0.35× echo at 40 ms is found");
        assert!((30..=60).contains(&e.delay_ms()) && (0.3..0.45).contains(&e.gain) && e.r > 0.8, "{e:?}");
        let e = detect(&with_echo(&user, &remote, 0.2, 1920), &remote).expect("a 0.2× echo at 120 ms is found");
        assert!((90..=150).contains(&e.delay_ms()) && (0.15..0.3).contains(&e.gain), "{e:?}");
        // Fainter than `MIN_GAIN` (an OS echo canceller's leftover): cancelling it would change nothing.
        assert_eq!(detect(&with_echo(&user, &remote, 0.03, 640), &remote), None);
        // Too little of the other side to decide on: no decision.
        let short = RATE as usize * 4;
        assert_eq!(detect(&with_echo(&user[..short], &remote[..short], 0.35, 640), &remote[..short]), None);
    }

    #[test]
    fn an_echo_is_found_when_the_user_talks_over_the_other_side_and_never_on_headphones() {
        // Talking over the other side drowned the echo's correlation (0.96 → 0.5): the second look leaves it out.
        let n = RATE as usize * 120;
        for seed in [3, 7, 19] {
            let (mut user, remote) = turn_taking(seed, n);
            talk_over(&mut user, &remote, seed + 1);
            let e = detect(&with_noise(&with_echo(&user, &remote, 0.35, 640), seed), &remote).expect("found over double talk");
            assert!((30..=60).contains(&e.delay_ms()) && (0.3..0.45).contains(&e.gain) && e.r > 0.8, "{e:?}");
            // Headphones, with the room's noise and the same talking over: no echo (the second look picks frames by
            // the mic's loudness, which must not make up a correlation).
            assert_eq!(detect(&with_noise(&user, seed), &remote), None, "seed {seed}");
            let (user, remote) = turn_taking(seed, n);
            assert_eq!(detect(&with_noise(&user, seed), &remote), None, "seed {seed}");
        }
    }

    /// One remote turn of `secs` (speech-like) between 1 s of silence, as (remote, the mic hearing it 0.35× 40 ms late).
    fn echo_turn(secs: usize) -> (Vec<f32>, Vec<f32>) {
        let mut x = 11u32;
        let mut next = move || {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            x
        };
        let mut remote = vec![0.0; RATE as usize];
        syllables(&mut next, &mut remote, RATE as usize * secs);
        remote.resize(remote.len() + RATE as usize, 0.0);
        let mic = with_echo(&vec![0.0; remote.len()], &remote, 0.35, 640);
        (remote, mic)
    }

    /// The user's own voice at a speaking level, added to `mic` from `at` s for `secs`.
    fn say(mic: &mut [f32], at: f32, secs: f32) {
        let mut x = 5u32;
        let start = (at * RATE as f32) as usize;
        for s in &mut mic[start..start + (secs * RATE as f32) as usize] {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *s += ((x >> 9) as f32 / (1u32 << 23) as f32 - 0.5) * 0.3;
        }
    }

    #[test]
    fn a_quiet_voice_of_the_users_own_at_the_echos_level_is_transcribed() {
        // The user talks over the other side no louder than its echo: only its loudness frame by frame tells it apart.
        for seed in 1..40u32 {
            let mut x = seed * 7919;
            let mut next = move || {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                x
            };
            let (mut remote, mut quiet) = (vec![0.0; RATE as usize], vec![0.0; RATE as usize]);
            syllables(&mut next, &mut remote, RATE as usize * 8);
            remote.resize(remote.len() + RATE as usize, 0.0);
            syllables(&mut next, &mut quiet, RATE as usize * 8);
            quiet.resize(remote.len(), 0.0);
            let quiet: Vec<f32> = quiet.iter().map(|s| s * 0.3).collect();
            let (em, er) = (envelope(&quiet), envelope(&remote));
            assert_eq!(ECHO.keep(&em, &er, 0, em.len()), vec![(0, em.len())], "seed {seed}");
        }
    }

    const FPS: f32 = RATE as f32 / ENV as f32;
    const ECHO: Echo = Echo { lag: 1, gain: 0.35, r: 0.95 };

    #[test]
    fn a_stretch_the_echo_explains_along_its_whole_length_is_not_transcribed() {
        let (remote, mic) = echo_turn(8);
        let (em, er) = (envelope(&mic), envelope(&remote));
        assert_eq!(ECHO.keep(&em, &er, 0, em.len()), vec![], "the echo alone is skipped");
        // The user talking over the other side from 2 to 7 s: those 5 s are kept whole, the echo around them isn't.
        let mut over = mic.clone();
        say(&mut over, 2.0, 5.0);
        let keep = ECHO.keep(&envelope(&over), &er, 0, em.len());
        assert_eq!(keep.len(), 1, "{keep:?}");
        let (a, b) = keep[0];
        assert!(a <= (2.0 * FPS) as usize && b >= (7.0 * FPS) as usize && b - a < (6.0 * FPS) as usize, "{keep:?}");
    }

    #[test]
    fn a_short_word_of_the_user_inside_a_long_echo_is_transcribed() {
        let (remote, mut mic) = echo_turn(10);
        say(&mut mic, 5.0, 0.3); // "yes", mid-echo
        let (em, er) = (envelope(&mic), envelope(&remote));
        let keep = ECHO.keep(&em, &er, 0, em.len());
        let (yes_from, yes_to) = ((5.0 * FPS) as usize, (5.3 * FPS) as usize);
        assert!(keep.iter().any(|&(a, b)| a <= yes_from && b >= yes_to), "the word is kept: {keep:?}");
        let kept: usize = keep.iter().map(|(a, b)| b - a).sum();
        assert!((kept as f32) < 1.5 * FPS, "only around the word: {kept} frames of {}", em.len());
    }

    #[test]
    fn a_word_said_at_the_echos_level_inside_a_long_echo_is_transcribed() {
        // A quiet "yes" no louder than the echo (#250 review): over the echo's band for its whole 300 ms, though
        // under `MARGIN`, and too short to fail `is_echo`'s nine frames in ten on its own.
        let (remote, mic) = echo_turn(10);
        let echo: Vec<f32> = with_echo(&vec![0.0; remote.len()], &remote, 0.35, 640);
        let er = envelope(&remote);
        for level in [1.0f32, 1.5] {
            for k in 0..30u32 {
                let at = 2.0 + k as f32 * 0.25;
                let (a, b) = ((at * RATE as f32) as usize, ((at + 0.3) * RATE as f32) as usize);
                // The word: as loud as `level` × the echo there, not correlated with it.
                let mut x = k + 1;
                let mut said = mic.clone();
                for i in a..b {
                    x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    said[i] += level * echo[i] * if x >> 31 == 0 { 1.0 } else { -1.0 };
                }
                let keep = ECHO.keep(&envelope(&said), &er, 0, er.len());
                let (wa, wb) = (a / ENV + 1, b / ENV - 1);
                assert!(keep.iter().any(|&(p, q)| p <= wa && q >= wb), "{level}× at {at} s is kept: {keep:?}");
                let kept: usize = keep.iter().map(|(p, q)| q - p).sum();
                assert!((kept as f32) < 1.5 * FPS, "{level}× at {at} s: only around the word, {kept} frames");
            }
        }
    }

    #[test]
    fn the_user_alone_and_audio_not_read_yet_are_transcribed() {
        // The other side quiet: whatever the mic hears is the user's.
        let mut mic = vec![0.0; RATE as usize * 4];
        say(&mut mic, 1.0, 2.0);
        let em = envelope(&mic);
        let er = vec![0.0; em.len()];
        assert_eq!(ECHO.keep(&em, &er, 0, em.len()), vec![(0, em.len())]);
        // The remote's envelope doesn't reach the end of the utterance yet: in doubt, all of it.
        let (remote, mic) = echo_turn(8);
        let (em, er) = (envelope(&mic), envelope(&remote));
        assert_eq!(ECHO.keep(&em, &er[..em.len() - 10], 0, em.len()), vec![(0, em.len())]);
    }

    #[test]
    fn echo_is_found_and_reduced() {
        let n = RATE as usize * 20;
        let remote = bursts(3, n);
        let delay = 1600; // 100 ms speaker → mic
        let mic: Vec<f32> = (0..n).map(|i| if i >= delay { remote[i - delay] * 0.5 } else { 0.0 }).collect();
        let found = detect(&mic, &remote).expect("echo detected").delay_ms();
        assert!((60..=150).contains(&found), "delay {found} ms");
        let (clean, _) = cancel(&mic, &remote).unwrap().unwrap();
        // After convergence (skip the first 3 s) most of the echo is gone.
        let tail = RATE as usize * 3;
        assert!(energy(&clean[tail..]) < energy(&mic[tail..]) * 0.5, "echo not reduced");
    }
}
