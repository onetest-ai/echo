//! Who was speaking, from Teams' own marker: the call window rings the speaker's tile and
//! avatar, and accessibility shows that as a class toggling on the element (recorded during the
//! call in `<call>/speaking.jsonl`, `ax::SpeakingLog`). Teams' class names are generated and
//! change between versions, so the marker is learned per call: the token whose presence on the
//! user's own tile follows their microphone speech — and on everyone else's follows the remote
//! track's speech — after shifting it back by the UI's delay (the ring lights ~0.5 s after the
//! voice; also learned). Tiles and the participants list are calibrated separately: they ring
//! with different classes.
//!
//! The result is a timeline of who was lit when; `speakers.rs` names remote words with it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::transcribe::Segment;

pub const FILE: &str = "speaking.jsonl";

/// Teams' own speaking marker as of 2026-10: the speaker's video tile gets a child element with
/// this class (the ring around the tile), whatever the UI language and without the
/// participants pane — seen in recorded trees on exactly the speaking person's tile. Preferred
/// when it fits the call; the calibration still measures the delay and finds a renamed marker
/// after a Teams update.
const KNOWN_MARKERS: &[&str] = &["vdi-frame-occlusion"];
/// The previous speaker lit again this soon gets the floor back.
const BOUNCE_S: f64 = 2.5;
/// A ring lit this much of this window is stuck, not speech.
const STUCK_WINDOW_S: f64 = 30.0;

/// A calibration is used only from this F1 (lit time vs speech time).
const MIN_SCORE: f64 = 0.45;
/// Teams' own ring (`KNOWN_MARKERS`) from this F1, when it lights one tile at a time.
const MIN_KNOWN_SCORE: f64 = 0.3;
/// Of a marker's lit time, at least this share with exactly one other person lit (measured on
/// 2026-10-29: the ring 0.88–0.96, layout classes and "video is on" 0.00–0.36).
const MIN_EXCLUSIVE: f64 = 0.6;
/// Each side needs this much speech to calibrate on.
const MIN_SPEECH_S: f64 = 15.0;
/// UI delays tried (seconds).
const MAX_LAG: f64 = 1.5;
/// Lit stretches shorter than this are flicker.
const MIN_LIT_S: f64 = 0.2;
/// A word belongs to whoever is lit around it (this much either side).
const WORD_PAD: f64 = 0.15;
/// Abstain when the runner-up has this share of the top overlap.
const ABSTAIN: f64 = 0.9;

type Spans = Vec<(f64, f64)>;

/// One participant's states over the call.
#[derive(Default)]
struct Track {
    name: String,
    me: bool,
    /// (time, tokens from then on); an empty set after the element went away.
    states: Vec<(f64, BTreeSet<String>)>,
}

impl Track {
    /// When `token` was present (until `end`).
    fn present(&self, token: &str, end: f64) -> Spans {
        let mut out: Spans = vec![];
        for (i, (t, set)) in self.states.iter().enumerate() {
            if set.contains(token) {
                let until = self.states.get(i + 1).map_or(end, |n| n.0);
                match out.last_mut() {
                    Some(l) if (l.1 - t).abs() < 1e-9 => l.1 = until,
                    _ => out.push((*t, until)),
                }
            }
        }
        out
    }

    /// Tokens that came and went during the call (a state marker, not a fixed style).
    fn toggled(&self) -> BTreeSet<String> {
        let mut seen: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for w in self.states.windows(2) {
            for t in w[1].1.difference(&w[0].1) {
                seen.entry(t).or_default().0 += 1;
            }
            for t in w[0].1.difference(&w[1].1) {
                seen.entry(t).or_default().1 += 1;
            }
        }
        // Element ids (`id:`) only serve the raised-hand check; they are never the marker.
        seen.into_iter().filter(|(t, (on, off))| *on >= 1 && *off >= 1 && !t.starts_with("id:")).map(|(t, _)| t.to_string()).collect()
    }

    /// When the hand was raised: an element id with "raisehand" (Teams' HTML), or the label
    /// saying so (English).
    fn hand_raised(&self, end: f64) -> Spans {
        let tokens: BTreeSet<&String> = self.states.iter().flat_map(|s| s.1.iter()).collect();
        let hand = |t: &str| {
            let t = t.to_lowercase();
            t.contains("raisehand") || (t.starts_with("t:") && t.contains("raised") && t.contains("hand"))
        };
        union(tokens.into_iter().filter(|t| hand(t)).flat_map(|t| self.present(t, end)).collect())
    }
}

fn load(dir: &Path) -> BTreeMap<String, Track> {
    let mut tracks: BTreeMap<String, Track> = BTreeMap::new();
    let Ok(text) = fs::read_to_string(dir.join(FILE)) else { return tracks };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let (Some(t), Some(k)) = (v["t"].as_f64(), v["k"].as_str()) else { continue };
        let tr = tracks.entry(k.to_string()).or_default();
        if let Some(n) = v["n"].as_str() {
            tr.name = n.to_string();
        }
        tr.me |= v["me"].as_bool().unwrap_or(false);
        let mut set = if v["gone"].as_bool() == Some(true) { BTreeSet::new() } else { tr.states.last().map(|s| s.1.clone()).unwrap_or_default() };
        let strs = |key: &str| v[key].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect::<Vec<_>>();
        for d in strs("del") {
            set.remove(&d);
        }
        set.extend(strs("add"));
        tr.states.push((t, set));
    }
    tracks
}

fn total(s: &Spans) -> f64 {
    s.iter().map(|(a, b)| (b - a).max(0.0)).sum()
}

fn union(mut s: Spans) -> Spans {
    s.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Spans = vec![];
    for (a, b) in s {
        match out.last_mut() {
            Some(l) if a <= l.1 => l.1 = l.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Time in both (each sorted and disjoint).
fn intersect(a: &Spans, b: &Spans) -> f64 {
    let (mut i, mut j, mut sum) = (0, 0, 0.0);
    while i < a.len() && j < b.len() {
        let o = a[i].1.min(b[j].1) - a[i].0.max(b[j].0);
        if o > 0.0 {
            sum += o;
        }
        if a[i].1 < b[j].1 { i += 1 } else { j += 1 }
    }
    sum
}

fn f1(lit: &Spans, speech: &Spans) -> f64 {
    let (l, s) = (total(lit), total(speech));
    if l + s <= 0.0 { 0.0 } else { 2.0 * intersect(lit, speech) / (l + s) }
}

fn shift(s: &Spans, by: f64) -> Spans {
    s.iter().map(|(a, b)| (a - by, b - by)).filter(|(a, b)| b - a >= MIN_LIT_S).collect()
}

/// Speech of one side from the transcript, joined across short gaps.
fn speech(segs: &[Segment], remote: bool) -> Spans {
    let mut s = union(segs.iter().filter(|x| (x.speaker == "remote") == remote).map(|x| (x.start, x.end)).collect());
    let mut out: Spans = vec![];
    for (a, b) in s.drain(..) {
        match out.last_mut() {
            Some(l) if a - l.1 < 0.5 => l.1 = b,
            _ => out.push((a, b)),
        }
    }
    out
}

/// Who was lit when (remote participants only), learned for one call.
pub struct Timeline {
    lit: BTreeMap<String, Spans>,
    /// What was learned, for the log: per element kind, (token, delay, score).
    pub learned: Vec<(String, String, f64, f64)>,
}

impl Timeline {
    /// Calibrate on the call's own speech; None when the log is missing or no marker fits.
    pub fn learn(dir: &Path, segs: &[Segment], me: Option<&str>) -> Option<Timeline> {
        let mut tracks = load(dir);
        if tracks.is_empty() {
            return None;
        }
        // The user: their own tile ("Myself video, <name>") names them for the participants
        // list too, which has no such marker; or the configured name.
        let mut mine: BTreeSet<String> = tracks.values().filter(|t| t.me).map(|t| t.name.to_lowercase()).collect();
        mine.extend(me.map(str::to_lowercase));
        for t in tracks.values_mut() {
            t.me |= mine.contains(&t.name.to_lowercase());
        }
        let end = segs.iter().map(|s| s.end).fold(0.0, f64::max).max(tracks.values().flat_map(|t| t.states.last()).map(|s| s.0).fold(0.0, f64::max)) + 1.0;
        let (mine, theirs) = (speech(segs, false), speech(segs, true));
        let mut lit: BTreeMap<String, Spans> = BTreeMap::new();
        let mut hands: BTreeMap<String, Spans> = BTreeMap::new();
        let mut learned = vec![];
        for kind in ["tile:", "roster:"] {
            let group: Vec<&Track> = tracks.iter().filter(|(k, _)| k.starts_with(kind)).map(|(_, t)| t).collect();
            let Some((token, lag, score)) = calibrate(&group, &mine, &theirs, end) else { continue };
            learned.push((kind.trim_end_matches(':').to_string(), token.clone(), lag, score));
            for t in group.iter().filter(|t| !t.me) {
                lit.entry(t.name.clone()).or_default().extend(shift(&t.present(&token, end), lag));
                hands.entry(t.name.clone()).or_default().extend(shift(&t.hand_raised(end), lag));
            }
        }
        if lit.is_empty() {
            return None;
        }
        let lit: BTreeMap<String, Spans> = lit.into_iter().map(|(n, s)| (n, union(s))).collect();
        let hands: BTreeMap<String, Spans> = hands.into_iter().map(|(n, s)| (n, union(s))).collect();
        Some(Timeline { lit: floor(&lit, &hands, &theirs, end), learned })
    }

    /// Who was lit during [a, b]: the participant with the most lit time around it; None when
    /// nobody was, or two were about equally (better "Remote" than a wrong name).
    pub fn who(&self, a: f64, b: f64) -> Option<&str> {
        let span = vec![(a - WORD_PAD, b + WORD_PAD)];
        let mut by: Vec<(&str, f64)> = self.lit.iter().map(|(n, s)| (n.as_str(), intersect(s, &span))).filter(|x| x.1 > 0.0).collect();
        by.sort_by(|x, y| y.1.total_cmp(&x.1));
        match by.as_slice() {
            [] => None,
            [(n, _)] => Some(n),
            [(n, top), (_, next), ..] => (*next < top * ABSTAIN).then_some(*n),
        }
    }
}

/// The floor: who holds it from moment to moment, when several tiles are lit at once.
/// - The holder keeps it while still lit (suspicion only decides who takes a free floor).
/// - The previous holder, lit again within `BOUNCE_S` of losing it, gets it back (a flicker
///   of someone else's ring).
/// - Otherwise the one lit most recently: a new voice.
/// - Suspect tiles take it only when nobody else is lit: a raised hand; a ring stuck on (lit
///   ≥ 90 % of the last 30 s); a tile lit at least as much in silence as in speech (a noisy
///   open mic).
fn floor(lit: &BTreeMap<String, Spans>, hands: &BTreeMap<String, Spans>, speech: &Spans, end: f64) -> BTreeMap<String, Spans> {
    let mut cuts: Vec<f64> = lit.values().chain(hands.values()).flatten().flat_map(|(a, b)| [*a, *b]).collect();
    cuts.sort_by(|a, b| a.total_cmp(b));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let covers = |s: &Spans, t: f64| s.iter().any(|(a, b)| *a <= t && t < *b);
    let silence = (end - total(speech)).max(0.0);
    let noisy: BTreeSet<&String> = lit
        .iter()
        .filter(|(_, s)| {
            let in_speech = intersect(s, speech);
            let in_silence = total(s) - in_speech;
            total(speech) >= 8.0 && silence >= 5.0 && in_silence / silence >= in_speech / total(speech).max(1e-9)
        })
        .map(|(n, _)| n)
        .collect();
    let mut out: BTreeMap<String, Spans> = BTreeMap::new();
    let (mut cur, mut prev, mut lost_at): (Option<&String>, Option<&String>, f64) = (None, None, f64::MIN);
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mid = (a + b) / 2.0;
        let lit_now: Vec<&String> = lit.iter().filter(|(_, s)| covers(s, mid)).map(|(n, _)| n).collect();
        let suspect = |n: &String| {
            let stuck = mid >= STUCK_WINDOW_S && intersect(&lit[n], &vec![(mid - STUCK_WINDOW_S, mid)]) >= 0.9 * STUCK_WINDOW_S;
            noisy.contains(n) || stuck || hands.get(n).is_some_and(|h| covers(h, mid))
        };
        let clean: Vec<&String> = lit_now.iter().copied().filter(|n| !suspect(n)).collect();
        let cands = if clean.is_empty() { lit_now.clone() } else { clean };
        let started = |n: &String| lit[n].iter().filter(|(s, _)| *s <= mid).map(|(s, _)| *s).fold(f64::MIN, f64::max);
        // The holder keeps the floor while lit, suspect or not (a long monologue looks "stuck").
        let holder = if cur.is_some_and(|c| lit_now.contains(&c)) {
            cur
        } else if prev.is_some_and(|p| cands.contains(&p)) && a - lost_at <= BOUNCE_S {
            prev
        } else {
            cands.iter().copied().max_by(|x, y| started(x).total_cmp(&started(y)))
        };
        if holder != cur {
            if cur.is_some() {
                prev = cur;
                lost_at = a;
            }
            cur = holder;
        }
        if let Some(h) = holder {
            let spans = out.entry(h.clone()).or_default();
            match spans.last_mut() {
                Some(l) if (l.1 - a).abs() < 1e-9 => l.1 = b,
                _ => spans.push((a, b)),
            }
        }
    }
    out
}

/// Of the time `token` is lit on any of these tracks, the share when exactly one is lit: a
/// speaking ring is on one tile at a time; a layout style or a state like "video is on" is on many.
fn exclusivity(tracks: &[&&Track], token: &str, end: f64) -> f64 {
    let mut edges: Vec<(f64, i32)> = vec![];
    for t in tracks {
        for (a, b) in union(t.present(token, end)) {
            edges.push((a, 1));
            edges.push((b, -1));
        }
    }
    edges.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let (mut n, mut last, mut one, mut any) = (0i32, 0.0f64, 0.0f64, 0.0f64);
    for (t, d) in edges {
        if n >= 1 {
            any += t - last;
        }
        if n == 1 {
            one += t - last;
        }
        n += d;
        last = t;
    }
    if any <= 0.0 { 0.0 } else { one / any }
}

/// The token and delay that best explain speech on one kind of element: the user's own
/// element against the mic's speech, the others' (together) against the remote track's.
///
/// With two or more others, a token must light one of them at a time (`exclusivity` ≥
/// `MIN_EXCLUSIVE`): against the remote track as a whole, a class that is on every tile (a layout
/// style) or on whoever has video ("video is on") fits as well as the ring when the other side
/// talks most of the call — it did, on two calls of 2026-10-29, and named everyone after one person.
fn calibrate(group: &[&Track], mine: &Spans, theirs: &Spans, end: f64) -> Option<(String, f64, f64)> {
    let selfs: Vec<&&Track> = group.iter().filter(|t| t.me).collect();
    let others: Vec<&&Track> = group.iter().filter(|t| !t.me).collect();
    let use_self = !selfs.is_empty() && total(mine) >= MIN_SPEECH_S;
    let use_others = !others.is_empty() && total(theirs) >= MIN_SPEECH_S;
    if !use_self && !use_others {
        return None;
    }
    let debug = std::env::var_os("ECHOCTL_DEBUG").is_some();
    let candidates: BTreeSet<String> = group.iter().flat_map(|t| t.toggled()).collect();
    // Per token: (its best delay, score, exclusivity).
    let mut scored: Vec<(String, f64, f64, f64)> = vec![];
    for token in candidates {
        let me_raw = union(selfs.iter().flat_map(|t| t.present(&token, end)).collect());
        let them_raw = union(others.iter().flat_map(|t| t.present(&token, end)).collect());
        if total(&me_raw) + total(&them_raw) <= 0.0 {
            continue;
        }
        let excl = if others.len() >= 2 { exclusivity(&others, &token, end) } else { 1.0 };
        let mut top: Option<(f64, f64)> = None;
        for step in 0..=(MAX_LAG * 10.0) as usize {
            let lag = step as f64 / 10.0;
            let mut scores = vec![];
            if use_self {
                scores.push(f1(&shift(&me_raw, lag), mine));
            }
            if use_others {
                scores.push(f1(&union(shift(&them_raw, lag)), theirs));
            }
            let score = scores.iter().sum::<f64>() / scores.len() as f64;
            if top.is_none_or(|t| score > t.1 + 1e-9) {
                top = Some((lag, score));
            }
        }
        if let Some((lag, score)) = top {
            scored.push((token, lag, score, excl));
        }
    }
    if debug {
        // `ECHOCTL_DEBUG=1 echoctl speaking <call>`: what each candidate scored.
        let mut list: Vec<&(String, f64, f64, f64)> = scored.iter().collect();
        list.sort_by(|a, b| b.2.total_cmp(&a.2));
        let shown = list.iter().take(8).chain(list.iter().filter(|x| KNOWN_MARKERS.contains(&x.0.as_str())));
        for (t, lag, score, excl) in shown {
            let known = if KNOWN_MARKERS.contains(&t.as_str()) { "  (known)" } else { "" };
            eprintln!("  candidate {t:<24} lag {lag:.1}s  F1 {score:.3}  one tile at a time {excl:.2}{known}");
        }
    }
    let pick = |want_known: bool| {
        scored
            .iter()
            .filter(|x| x.3 >= MIN_EXCLUSIVE && (!want_known || KNOWN_MARKERS.contains(&x.0.as_str())))
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|x| (x.0.clone(), x.1, x.2))
    };
    // Teams' ring, when it lights one tile at a time: its fit can be low (a screen share hides the
    // tiles for half the call) while it is still right, and a class that fits better can be its
    // opposite (on exactly while the ring is off, on 2026-10-29). Another class only when the ring
    // isn't there (renamed by a Teams update).
    match pick(true) {
        Some(k) if k.2 >= MIN_KNOWN_SCORE => Some(k),
        _ => pick(false).filter(|b| b.2 >= MIN_SCORE),
    }
}

/// The one other person in the call, when Teams showed exactly one: a remote video tile, or a
/// participants-list row that is in the call (its label says muted / unmuted — invitees who
/// haven't joined are listed too, "No response"). The invite can't tell (six invited, two came).
pub fn sole_remote(dir: &Path, me: Option<&str>) -> Option<String> {
    let tracks = load(dir);
    let mine: BTreeSet<String> = tracks.values().filter(|t| t.me).map(|t| t.name.to_lowercase()).chain(me.map(str::to_lowercase)).collect();
    let present = |k: &str, t: &Track| {
        k.starts_with("tile:") || t.states.iter().any(|(_, s)| s.contains("t:muted") || s.contains("t:unmuted"))
    };
    let others: BTreeSet<&str> = tracks.iter().filter(|(k, t)| !t.me && !mine.contains(&t.name.to_lowercase()) && present(k, t)).map(|(_, t)| t.name.as_str()).collect();
    match others.into_iter().collect::<Vec<_>>().as_slice() {
        [one] => Some(one.to_string()),
        _ => None,
    }
}

/// `echoctl speaking <call>`: the marker learned for a recorded call and who it lights, per
/// remote segment.
pub fn report(call: &str) -> anyhow::Result<()> {
    let dir = crate::calls::resolve_call(call)?;
    let segs: Vec<Segment> = serde_json::from_slice(&fs::read(dir.join("segments.json"))?)?;
    let me = crate::config::Config::load().ok().and_then(|c| c.me).map(|m| m.name);
    let tracks = load(&dir);
    println!("{} participants in {FILE}", tracks.len());
    for (k, t) in &tracks {
        println!("  {}{} [{k}] {} states, toggled: {}", t.name, if t.me { " (me)" } else { "" }, t.states.len(), t.toggled().len());
    }
    let Some(tl) = Timeline::learn(&dir, &segs, me.as_deref()) else {
        println!("no speaking marker fits this call");
        return Ok(());
    };
    for (kind, token, lag, score) in &tl.learned {
        println!("marker ({kind}s): {token}, {lag:.1}s behind the voice, fit {score:.2}");
    }
    for s in segs.iter().filter(|s| s.speaker == "remote").take(40) {
        println!("  [{:7.1}] {:<24} {}", s.start, tl.who(s.start, s.end).unwrap_or("?"), s.text.chars().take(70).collect::<String>());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(speaker: &str, start: f64, end: f64) -> Segment {
        Segment { speaker: speaker.into(), start, end, lang: "en".into(), text: "x".into(), name: None, speaker_id: None, words: vec![] }
    }

    #[test]
    fn the_speaking_marker_and_the_ui_delay_are_learned_from_the_call() {
        let dir = std::env::temp_dir().join(format!("echo-speaking-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // Me speaks 0–10, 40–50; Dana 10–25; Lee 25–40 and 50–60. The ring ("___ring") lights
        // 0.5 s late; "t:unmuted" stays on for everyone (not a speaking marker).
        let mut lines = vec![];
        let tile = |k: &str, n: &str, me: bool| (format!("tile:{k}"), n.to_string(), me);
        let (jane, dana, lee) = (tile("Jane", "Jane Doe", true), tile("Dana", "Dana Lin", false), tile("Lee", "Lee Park", false));
        let mut push = |who: &(String, String, bool), t: f64, add: &[&str], del: &[&str]| {
            let mut v = serde_json::json!({ "t": t, "k": who.0, "n": who.1, "add": add, "del": del });
            if who.2 {
                v["me"] = true.into();
            }
            lines.push(v.to_string());
        };
        for w in [&jane, &dana, &lee] {
            push(w, 0.0, &["tile", "t:unmuted"], &[]);
        }
        for (who, a, b) in [(&jane, 0.0, 10.0), (&dana, 10.0, 25.0), (&lee, 25.0, 40.0), (&jane, 40.0, 50.0), (&lee, 50.0, 60.0)] {
            push(who, a + 0.5, &["___ring"], &[]);
            push(who, b + 0.5, &[], &["___ring"]);
        }
        fs::write(dir.join(FILE), lines.join("\n")).unwrap();
        let segs = vec![seg("me", 0.0, 10.0), seg("remote", 10.0, 25.0), seg("remote", 25.0, 40.0), seg("me", 40.0, 50.0), seg("remote", 50.0, 60.0)];
        let tl = Timeline::learn(&dir, &segs, None).expect("calibrated");
        assert_eq!(tl.learned[0].1, "___ring");
        assert!((tl.learned[0].2 - 0.5).abs() < 1e-9, "{:?}", tl.learned);
        assert_eq!(tl.who(12.0, 13.0), Some("Dana Lin"));
        assert_eq!(tl.who(30.0, 31.0), Some("Lee Park"));
        assert_eq!(tl.who(5.0, 6.0), None, "the user's own tile never names remote speech");
        assert_eq!(sole_remote(&dir, None), None, "two others were on screen");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn teams_known_ring_is_preferred_over_a_lookalike() {
        let dir = std::env::temp_dir().join(format!("echo-known-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // Two tokens follow Dana's speech; the known marker is chosen, delay learned.
        let mut lines = vec![];
        for (k, n) in [("tile:Dana", "Dana Lin"), ("tile:Lee", "Lee Park")] {
            lines.push(serde_json::json!({ "t": 0.0, "k": k, "n": n, "add": ["tile"], "del": [] }).to_string());
        }
        for (k, n, a, b) in [("tile:Dana", "Dana Lin", 0.0, 20.0), ("tile:Lee", "Lee Park", 20.0, 40.0)] {
            lines.push(serde_json::json!({ "t": a + 0.4, "k": k, "n": n, "add": ["vdi-frame-occlusion", "___x"], "del": [] }).to_string());
            lines.push(serde_json::json!({ "t": b + 0.4, "k": k, "n": n, "add": [], "del": ["vdi-frame-occlusion", "___x"] }).to_string());
        }
        fs::write(dir.join(FILE), lines.join("\n")).unwrap();
        let tl = Timeline::learn(&dir, &[seg("remote", 0.0, 20.0), seg("remote", 20.0, 40.0)], None).unwrap();
        assert_eq!(tl.learned[0].1, "vdi-frame-occlusion");
        assert!((tl.learned[0].2 - 0.4).abs() < 1e-9);
        assert_eq!(tl.who(5.0, 6.0), Some("Dana Lin"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_class_on_every_tile_never_beats_the_ring_however_well_it_fits() {
        // 2026-10-29: the other side talked almost the whole call. A layout class on every tile
        // ("___layout", toggled when the tiles were redrawn) and "t:video is on" matched the
        // remote track better than the ring, which a screen share hid half the time — and every
        // voice was named after one person.
        let dir = std::env::temp_dir().join(format!("echo-exclusive-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut lines = vec![];
        let who = [("tile:Dana", "Dana Lin"), ("tile:Lee", "Lee Park"), ("tile:Kim", "Kim Ito")];
        for (k, n) in who {
            lines.push(serde_json::json!({ "t": 0.0, "k": k, "n": n, "add": ["tile", "___layout", "t:video is on"], "del": [] }).to_string());
            // Redrawn once: the layout class and the video state come and go on every tile.
            lines.push(serde_json::json!({ "t": 50.0, "k": k, "n": n, "add": [], "del": ["___layout", "t:video is on"] }).to_string());
            lines.push(serde_json::json!({ "t": 51.0, "k": k, "n": n, "add": ["___layout", "t:video is on"], "del": [] }).to_string());
        }
        // The ring: Dana 0–30, Lee 30–60, Kim 60–90 — but hidden by a share from 70 to 90.
        for (i, (k, n)) in who.iter().enumerate() {
            let (a, b) = (i as f64 * 30.0, if i == 2 { 70.0 } else { i as f64 * 30.0 + 30.0 });
            lines.push(serde_json::json!({ "t": a + 0.3, "k": k, "n": n, "add": ["vdi-frame-occlusion"], "del": [] }).to_string());
            lines.push(serde_json::json!({ "t": b + 0.3, "k": k, "n": n, "add": [], "del": ["vdi-frame-occlusion"] }).to_string());
        }
        fs::write(dir.join(FILE), lines.join("\n")).unwrap();
        let segs = [seg("remote", 0.0, 30.0), seg("remote", 30.0, 60.0), seg("remote", 60.0, 90.0)];
        let tl = Timeline::learn(&dir, &segs, None).expect("calibrated");
        assert_eq!(tl.learned[0].1, "vdi-frame-occlusion", "{:?}", tl.learned);
        assert_eq!(tl.who(10.0, 11.0), Some("Dana Lin"));
        assert_eq!(tl.who(40.0, 41.0), Some("Lee Park"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_floor_keeps_its_holder_bounces_back_and_skips_suspect_tiles() {
        let m = |v: &[(&str, &[(f64, f64)])]| v.iter().map(|(n, s)| (n.to_string(), s.to_vec())).collect::<BTreeMap<_, _>>();
        let speech: Spans = vec![(0.0, 100.0)];
        // Dana speaks 0–20; Lee's ring flickers 10–11 (Dana still lit) — Dana keeps it.
        // Lee takes over at 20; Dana flickers at 21–22 — Lee keeps it.
        // Lee stops 40; Kim lit 40–41, Lee lit again 41.5–60: bounce back to Lee, Kim's blip is
        // a second.
        let lit = m(&[("Dana", &[(0.0, 20.0), (21.0, 22.0)]), ("Lee", &[(10.0, 11.0), (20.0, 40.0), (41.5, 60.0)]), ("Kim", &[(40.0, 41.0), (60.0, 61.0)])]);
        let f = floor(&lit, &BTreeMap::new(), &speech, 100.0);
        let tl = Timeline { lit: f, learned: vec![] };
        assert_eq!(tl.who(10.2, 10.8), Some("Dana"));
        assert_eq!(tl.who(21.2, 21.8), Some("Lee"));
        assert_eq!(tl.who(45.0, 46.0), Some("Lee"));
        // A raised hand lit alongside the speaker never takes the floor from them.
        let lit = m(&[("Dana", &[(0.0, 50.0)]), ("Raj", &[(5.0, 50.0)])]);
        let hands = m(&[("Raj", &[(4.0, 50.0)])]);
        let tl = Timeline { lit: floor(&lit, &hands, &speech, 100.0), learned: vec![] };
        assert_eq!(tl.who(20.0, 21.0), Some("Dana"));
        // A long monologue keeps the floor past the stuck-ring window while others flicker.
        let lit = m(&[("Dana", &[(0.0, 90.0)]), ("Lee", &[(50.0, 51.0)])]);
        let tl = Timeline { lit: floor(&lit, &BTreeMap::new(), &speech, 100.0), learned: vec![] };
        assert_eq!(tl.who(50.2, 50.8), Some("Dana"));
    }

    #[test]
    fn the_only_other_person_on_screen_is_the_remote_side() {
        let dir = std::env::temp_dir().join(format!("echo-sole-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let lines = [
            r#"{"t":0.5,"k":"tile:Jane Doe","n":"Jane Doe","me":true,"add":["a"],"del":[]}"#,
            r#"{"t":0.5,"k":"tile:Kate Ortiz","n":"Kate Ortiz","add":["b"],"del":[]}"#,
            r#"{"t":9.0,"k":"roster:1","n":"Lee Park","add":["t:in a meeting","t:no response"],"del":[]}"#,
            r#"{"t":9.0,"k":"roster:2","n":"Jane Doe","add":["t:unmuted"],"del":[]}"#,
        ];
        fs::write(dir.join(FILE), lines.join("\n")).unwrap();
        assert_eq!(sole_remote(&dir, None).as_deref(), Some("Kate Ortiz"));
        let _ = fs::remove_dir_all(&dir);
    }
}
