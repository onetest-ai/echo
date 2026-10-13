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

/// A calibration is used only from this F1 (lit time vs speech time).
const MIN_SCORE: f64 = 0.45;
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
        seen.into_iter().filter(|(_, (on, off))| *on >= 1 && *off >= 1).map(|(t, _)| t.to_string()).collect()
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
        let mut learned = vec![];
        for kind in ["tile:", "roster:"] {
            let group: Vec<&Track> = tracks.iter().filter(|(k, _)| k.starts_with(kind)).map(|(_, t)| t).collect();
            let Some((token, lag, score)) = calibrate(&group, &mine, &theirs, end) else { continue };
            learned.push((kind.trim_end_matches(':').to_string(), token.clone(), lag, score));
            for t in group.iter().filter(|t| !t.me) {
                lit.entry(t.name.clone()).or_default().extend(shift(&t.present(&token, end), lag));
            }
        }
        if lit.is_empty() {
            return None;
        }
        let lit = lit.into_iter().map(|(n, s)| (n, union(s))).collect();
        Some(Timeline { lit, learned })
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

/// The token and delay that best explain speech on one kind of element: the user's own
/// element against the mic's speech, the others' (together) against the remote track's.
fn calibrate(group: &[&Track], mine: &Spans, theirs: &Spans, end: f64) -> Option<(String, f64, f64)> {
    let selfs: Vec<&&Track> = group.iter().filter(|t| t.me).collect();
    let others: Vec<&&Track> = group.iter().filter(|t| !t.me).collect();
    let use_self = !selfs.is_empty() && total(mine) >= MIN_SPEECH_S;
    let use_others = !others.is_empty() && total(theirs) >= MIN_SPEECH_S;
    if !use_self && !use_others {
        return None;
    }
    let candidates: BTreeSet<String> = group.iter().flat_map(|t| t.toggled()).collect();
    let mut best: Option<(String, f64, f64)> = None;
    for token in candidates {
        let me_raw = union(selfs.iter().flat_map(|t| t.present(&token, end)).collect());
        let them_raw = union(others.iter().flat_map(|t| t.present(&token, end)).collect());
        // A style that is simply on (a layout class) is lit nearly all the time.
        if total(&me_raw) + total(&them_raw) <= 0.0 {
            continue;
        }
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
            if best.as_ref().is_none_or(|b| score > b.2 + 1e-9) {
                best = Some((token.clone(), lag, score));
            }
        }
    }
    best.filter(|b| b.2 >= MIN_SCORE)
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
