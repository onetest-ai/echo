//! Who is speaking on the remote track. One track carries everyone else on the call, so a
//! group call's transcript said only "Remote". When the speaker model is downloaded
//! (`speakers-nemotron-3`: NVIDIA's Sortformer through NeMo-Speech, `nemo.rs`), the remote
//! track is diarized once after the call and each remote segment gets the speaker whose turn
//! overlaps it most: "Speaker 1", "Speaker 2", … numbered by who speaks first. It works the
//! same whichever engine transcribed the call (it reads the audio, not the text).
//!
//! Names come first from Teams itself: it rings whoever speaks, and `speaking.rs` turns that
//! into a timeline (see `label`). Voices it can't name stay "Speaker N" for the notes run or the
//! user: `echoctl speakers <call> 2="Daniel Osei"` writes `<call>/speakers.json`
//! (`{"2": "Daniel Osei"}`) and re-renders the transcript. A 1:1 call is still named from its
//! invite and is never split.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::models;
use crate::transcribe::{load_16k, Segment, RATE};

pub const FILE: &str = "speakers.json";

fn names(dir: &Path) -> BTreeMap<u32, String> {
    let raw: BTreeMap<String, String> = fs::read(dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    raw.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).filter(|(_, v): &(u32, String)| !v.trim().is_empty()).collect()
}

fn display(id: u32, names: &BTreeMap<u32, String>) -> String {
    names.get(&id).cloned().unwrap_or_else(|| format!("Speaker {id}"))
}

/// Name the remote side of a group call. Two sources, either or both:
/// - Teams' own speaking marker (`speaking.rs`, from `<call>/speaking.jsonl`) names words
///   directly;
/// - the speaker model (when downloaded) splits the track by voice: each voice gets a manual
///   name (`speakers.json`), else the name its words were lit for (`vote`), else "Speaker N".
///
/// Without the model, segments are named from the timeline alone.
pub fn label(dir: &Path, segs: &mut Vec<Segment>, me: Option<&str>) -> Result<()> {
    if !segs.iter().any(|s| s.speaker == "remote") {
        return Ok(());
    }
    // Only one other person in the call (whatever the invite says): the remote track is them.
    if let Some(name) = crate::speaking::sole_remote(dir, me) {
        eprintln!("  remote side: {name} (the only other person in the call)");
        segs.iter_mut().filter(|s| s.speaker == "remote").for_each(|s| s.name = Some(name.clone()));
        return Ok(());
    }
    let timeline = crate::speaking::Timeline::learn(dir, segs, me);
    if let Some(tl) = &timeline {
        for (kind, token, lag, score) in &tl.learned {
            eprintln!("  teams speaking marker ({kind}s): {token}, {lag:.1}s behind the voice (fit {score:.2})");
        }
    }
    let Some(turns) = diarize(dir)? else {
        if let Some(tl) = &timeline {
            split_by_timeline(segs, tl);
        }
        return Ok(());
    };
    let n = assign(segs, &turns);
    let manual = names(dir);
    let voted = timeline.as_ref().map(|tl| vote(segs, tl)).unwrap_or_default();
    if n <= 1 && manual.is_empty() && voted.is_empty() {
        // One voice (or none found): "Remote" says as much as "Speaker 1".
        segs.iter_mut().for_each(|s| s.speaker_id = None);
    } else {
        for s in segs.iter_mut().filter(|s| s.speaker == "remote") {
            if let Some(id) = s.speaker_id {
                s.name = Some(manual.get(&id).or(voted.get(&id)).cloned().unwrap_or_else(|| format!("Speaker {id}")));
            }
        }
    }
    eprintln!("  speakers: {n} on the remote track, {} named from Teams", voted.len());
    Ok(())
}

/// The speaker model's turns on `<dir>/remote.wav` (None without the model or the audio).
fn diarize(dir: &Path) -> Result<Option<Vec<crate::nemo::Turn>>> {
    let Some(model) = models::speakers_model() else { return Ok(None) };
    let wav = dir.join("remote.wav");
    if !wav.exists() {
        return Ok(None);
    }
    let t = Instant::now();
    let gpu = cfg!(target_os = "macos") && crate::gpu::allowed();
    let diarizer = crate::nemo::Diarizer::load(&model, gpu)?;
    let turns = diarizer.run(pieces(&wav)?)?;
    if std::env::var_os("ECHOCTL_DEBUG").is_some() {
        eprintln!("  debug: speaker turns {}", turns.iter().map(|(a, b, w)| format!("{w}:{a:.1}-{b:.1}")).collect::<Vec<_>>().join(" "));
    }
    eprintln!("  speaker model: {:.1}s", t.elapsed().as_secs_f64());
    Ok(Some(turns))
}

/// The WAV in 10 s pieces (the recorder writes 16 kHz mono 16-bit; anything else is converted
/// whole), then an empty piece.
fn pieces(wav: &Path) -> Result<impl FnMut() -> Result<Vec<f32>>> {
    let reader = hound::WavReader::open(wav).with_context(|| format!("reading {}", wav.display()))?;
    let spec = reader.spec();
    let native = spec.sample_rate == RATE as u32 && spec.channels == 1 && spec.bits_per_sample == 16 && spec.sample_format == hound::SampleFormat::Int;
    let mut whole = if native { None } else { Some(load_16k(wav)?) };
    let mut samples = reader.into_samples::<i16>();
    let mut done = 0;
    Ok(move || -> Result<Vec<f32>> {
        if let Some(all) = whole.as_mut() {
            let piece = all[done.min(all.len())..(done + 10 * RATE).min(all.len())].to_vec();
            done += piece.len();
            return Ok(piece);
        }
        let mut piece = Vec::with_capacity(10 * RATE);
        for s in samples.by_ref().take(10 * RATE) {
            piece.push(s? as f32 / i16::MAX as f32);
        }
        Ok(piece)
    })
}

/// Speakers whose turns overlap a stretch of time.
fn overlapping(start: f64, end: f64, turns: &[crate::nemo::Turn]) -> Vec<u32> {
    turns.iter().filter(|(a, b, _)| b.min(end) > a.max(start)).map(|t| t.2).collect()
}

/// The diarizer's speaker for a stretch of time: the one whose turns overlap it most, else the
/// turn nearest its middle (within half a second: a word between two turns).
fn who(start: f64, end: f64, turns: &[crate::nemo::Turn]) -> Option<u32> {
    // Per speaker: overlap, and the latest start of their turns overlapping it.
    let mut by: BTreeMap<u32, (f64, f64)> = BTreeMap::new();
    for (a, b, w) in turns {
        let o = b.min(end) - a.max(start);
        if o > 0.0 {
            let e = by.entry(*w).or_insert((0.0, f64::MIN));
            e.0 += o;
            e.1 = e.1.max(*a);
        }
    }
    let mut ranked: Vec<(u32, (f64, f64))> = by.into_iter().collect();
    ranked.sort_by(|x, y| y.1 .0.total_cmp(&x.1 .0));
    if let Some(&(w, (o, _))) = ranked.first() {
        // Both talking (overlapping turns cover the word alike): the one who just started
        // is the interjection being transcribed — the other was already mid-sentence.
        let tied = ranked.iter().take_while(|r| r.1 .0 >= o * 0.9);
        return tied.max_by(|x, y| x.1 .1.total_cmp(&y.1 .1)).map(|r| r.0).or(Some(w));
    }
    let mid = (start + end) / 2.0;
    let gap = |(a, b, _): &crate::nemo::Turn| if mid < *a { a - mid } else if mid > *b { mid - b } else { 0.0 };
    turns.iter().filter(|t| gap(t) <= 0.5).min_by(|x, y| gap(x).total_cmp(&gap(y))).map(|t| t.2)
}

/// Split one remote segment where the speaker changes between its words (two people's words
/// in one decoded sentence). A one-word run the neighbour's turn also covers joins it.
/// Returns the pieces with the diarizer's own speaker ids.
fn split(seg: Segment, turns: &[crate::nemo::Turn]) -> Vec<(Segment, Option<u32>)> {
    let words: Vec<&str> = seg.text.split_whitespace().collect();
    if seg.words.len() < 2 || seg.words.len() != words.len() {
        let w = who(seg.start, seg.end, turns);
        return vec![(seg, w)];
    }
    let mut ids: Vec<Option<u32>> = seg.words.iter().map(|[a, b]| who(*a, *b, turns)).collect();
    // Unknown words take the speaker before them (or after, at the start).
    for i in 1..ids.len() {
        if ids[i].is_none() {
            ids[i] = ids[i - 1];
        }
    }
    for i in (0..ids.len().saturating_sub(1)).rev() {
        if ids[i].is_none() {
            ids[i] = ids[i + 1];
        }
    }
    // A run of one word whose time also overlaps the neighbouring speaker goes with that
    // neighbour (a turn edge a word off); a clear one-word turn ("Hello?") stays.
    let ambiguous = |i: usize, with: Option<u32>| with.is_some_and(|w| overlapping(seg.words[i][0], seg.words[i][1], turns).contains(&w));
    let mut runs: Vec<(usize, usize, Option<u32>)> = vec![];
    for (i, id) in ids.iter().enumerate() {
        match runs.last_mut() {
            Some(r) if r.2 == *id => r.1 = i + 1,
            _ => runs.push((i, i + 1, *id)),
        }
    }
    let mut merged: Vec<(usize, usize, Option<u32>)> = vec![];
    for r in runs {
        let first = merged.len() == 1;
        match merged.last_mut() {
            Some(m) if r.1 - r.0 == 1 && ambiguous(r.0, m.2) => m.1 = r.1,
            Some(m) if m.1 - m.0 == 1 && first && ambiguous(m.0, r.2) => *m = (m.0, r.1, r.2),
            Some(m) if m.2 == r.2 => m.1 = r.1,
            _ => merged.push(r),
        }
    }
    cut(seg, merged)
}

/// Cut a segment into word runs `(from, to, label)`; one run keeps the segment whole.
fn cut<T: Clone>(seg: Segment, runs: Vec<(usize, usize, Option<T>)>) -> Vec<(Segment, Option<T>)> {
    if runs.len() == 1 {
        let w = runs[0].2.clone();
        return vec![(seg, w)];
    }
    let words: Vec<&str> = seg.text.split_whitespace().collect();
    runs.into_iter()
        .map(|(i, j, id)| {
            let piece = Segment {
                start: seg.words[i][0],
                end: seg.words[j - 1][1],
                text: words[i..j].join(" "),
                words: seg.words[i..j].to_vec(),
                ..seg.clone()
            };
            (piece, id)
        })
        .collect()
}

/// Words within this of the previous one continue its speaker (one voice unit).
const UNIT_GAP: f64 = 0.7;

/// No speaker model: name remote words straight from Teams' timeline, splitting a segment
/// where the lit person changes. A word nobody was lit for continues the word before it when
/// it follows closely; otherwise it stays unnamed ("Remote").
fn split_by_timeline(segs: &mut Vec<Segment>, tl: &crate::speaking::Timeline) {
    let mut out = vec![];
    for seg in segs.drain(..) {
        if seg.speaker != "remote" {
            out.push(seg);
            continue;
        }
        let aligned = !seg.words.is_empty() && seg.words.len() == seg.text.split_whitespace().count();
        if !aligned {
            let name = tl.who(seg.start, seg.end).map(str::to_owned);
            out.push(Segment { name, ..seg });
            continue;
        }
        let mut labels: Vec<Option<String>> = seg.words.iter().map(|[a, b]| tl.who(*a, *b).map(str::to_owned)).collect();
        for i in 1..labels.len() {
            if labels[i].is_none() && seg.words[i][0] - seg.words[i - 1][1] < UNIT_GAP {
                labels[i] = labels[i - 1].clone();
            }
        }
        let mut runs: Vec<(usize, usize, Option<String>)> = vec![];
        for (i, l) in labels.into_iter().enumerate() {
            match runs.last_mut() {
                Some(r) if r.2 == l => r.1 = i + 1,
                _ => runs.push((i, i + 1, l)),
            }
        }
        for (mut piece, name) in cut(seg, runs) {
            piece.name = name;
            out.push(piece);
        }
    }
    *segs = out;
}

/// Names for detected speakers from Teams' timeline: a voice takes the name most of its words
/// were lit for — at least 3 words, at least 60 % of its named words, and named words at least
/// a quarter of all its words. Several voices on one name keep that name: the speaker model splits
/// one person into several voices far more often than two people share a room's tile (a 3-person
/// call came out as 8 voices on 2026-10-29, "Pat (Speaker 1)" … "(Speaker 4)").
fn vote(segs: &[Segment], tl: &crate::speaking::Timeline) -> BTreeMap<u32, String> {
    let mut words: BTreeMap<u32, (usize, BTreeMap<String, usize>)> = BTreeMap::new();
    for s in segs.iter().filter(|s| s.speaker == "remote") {
        let Some(id) = s.speaker_id else { continue };
        let e = words.entry(id).or_default();
        let n = s.text.split_whitespace().count();
        e.0 += n;
        if !s.words.is_empty() && s.words.len() == n {
            for [a, b] in &s.words {
                if let Some(name) = tl.who(*a, *b) {
                    *e.1.entry(name.to_string()).or_default() += 1;
                }
            }
        } else if let Some(name) = tl.who(s.start, s.end) {
            *e.1.entry(name.to_string()).or_default() += n;
        }
    }
    let mut out: BTreeMap<u32, String> = BTreeMap::new();
    for (id, (all, by)) in words {
        let named: usize = by.values().sum();
        if let Some((name, top)) = by.into_iter().max_by_key(|x| x.1) {
            if top >= 3 && top as f64 >= 0.6 * named as f64 && named as f64 >= 0.25 * all as f64 {
                out.insert(id, name);
            }
        }
    }
    out
}

/// Give each remote segment its speaker (splitting segments two people share), numbered 1, 2,
/// … by first appearance. Returns how many speakers the segments ended up with.
fn assign(segs: &mut Vec<Segment>, turns: &[crate::nemo::Turn]) -> usize {
    let mut out: Vec<(Segment, Option<u32>)> = vec![];
    for s in segs.drain(..) {
        if s.speaker == "remote" {
            out.extend(split(s, turns));
        } else {
            out.push((s, None));
        }
    }
    let mut firsts: Vec<(f64, u32)> = vec![];
    for (s, raw) in &out {
        if let Some(r) = raw {
            match firsts.iter_mut().find(|f| f.1 == *r) {
                Some(f) => f.0 = f.0.min(s.start),
                None => firsts.push((s.start, *r)),
            }
        }
    }
    firsts.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (mut s, raw) in out {
        s.speaker_id = raw.and_then(|r| firsts.iter().position(|f| f.1 == r)).map(|n| n as u32 + 1);
        segs.push(s);
    }
    firsts.len()
}

/// Each voice's name again: a manual one (`speakers.json`), else the one Teams lit for it now
/// (`speaking.rs`, re-learned: the calibration may have changed since the call), else "Speaker N".
fn relabel(dir: &Path, segs: &mut [Segment]) -> BTreeMap<u32, String> {
    let manual = names(dir);
    let voted = crate::speaking::Timeline::learn(dir, segs, None).map(|tl| {
        for (kind, token, lag, score) in &tl.learned {
            eprintln!("  teams speaking marker ({kind}s): {token}, {lag:.1}s behind the voice (fit {score:.2})");
        }
        vote(segs, &tl)
    }).unwrap_or_default();
    let mut shown = BTreeMap::new();
    for s in segs.iter_mut() {
        if let Some(id) = s.speaker_id {
            let name = manual.get(&id).or(voted.get(&id)).cloned().unwrap_or_else(|| format!("Speaker {id}"));
            shown.insert(id, name.clone());
            s.name = Some(name);
        }
    }
    shown
}

/// `echoctl speakers <call>`: who spoke, how long, and their first lines (for naming them);
/// with `N=Name` pairs, name them (`N=` clears a name) and re-render the transcript; with
/// `--relabel`, name the voices from Teams again (after a fix to how its marker is learned).
pub fn command(call: &str, assignments: &[String], relabel_only: bool) -> Result<()> {
    let dir = crate::calls::resolve_call(call)?;
    let path = dir.join("segments.json");
    let mut segs: Vec<Segment> = serde_json::from_slice(&fs::read(&path).with_context(|| format!("{} has no transcript yet", dir.display()))?)?;
    if !segs.iter().any(|s| s.speaker_id.is_some()) {
        bail!("this call has no detected speakers (the speaker model wasn't downloaded when it was transcribed, or one voice was heard)");
    }
    if assignments.is_empty() && !relabel_only {
        let mut out: BTreeMap<u32, (f64, Vec<serde_json::Value>)> = BTreeMap::new();
        let mut shown: BTreeMap<u32, String> = BTreeMap::new();
        for s in segs.iter().filter(|s| s.speaker_id.is_some()) {
            let id = s.speaker_id.unwrap_or(0);
            if let Some(n) = &s.name {
                shown.entry(id).or_insert_with(|| n.clone());
            }
            let e = out.entry(id).or_default();
            e.0 += s.end - s.start;
            if e.1.len() < 4 {
                e.1.push(json!({ "t": s.start.round(), "text": s.text }));
            }
        }
        let names = names(&dir);
        // `label`: what the transcript shows (a manual name, the name Teams lit, or "Speaker N");
        // `name`: set by hand (`speakers.json`).
        let list: Vec<_> = out.into_iter().map(|(id, (secs, lines))| json!({
            "id": id, "name": names.get(&id), "label": shown.get(&id).cloned().unwrap_or_else(|| display(id, &names)),
            "seconds": secs.round(), "first_lines": lines,
        })).collect();
        println!("{}", serde_json::to_string_pretty(&json!({ "speakers": list }))?);
        return Ok(());
    }
    let mut raw: BTreeMap<String, String> = fs::read(dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    for a in assignments {
        let Some((id, name)) = a.split_once('=') else { bail!("expected N=Name, got '{a}'") };
        let id: u32 = id.trim().parse().with_context(|| format!("'{id}' is not a speaker number"))?;
        if !segs.iter().any(|s| s.speaker_id == Some(id)) {
            bail!("no speaker {id} in this call");
        }
        let name = name.trim().trim_matches('"').trim();
        if name.is_empty() {
            raw.remove(&id.to_string());
        } else {
            raw.insert(id.to_string(), name.to_string());
        }
    }
    if !assignments.is_empty() {
        fs::write(dir.join(FILE), serde_json::to_vec_pretty(&raw)?)?;
    }
    // Every voice, not only the ones named here: a manual name, else Teams', else "Speaker N".
    let names = relabel(&dir, &mut segs);
    fs::write(&path, serde_json::to_vec_pretty(&segs)?)?;
    let md = dir.join("transcript.md");
    let model = fs::read_to_string(&md).ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("- **Model:** ").map(str::to_owned))).unwrap_or_default();
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json"))?)?;
    let meeting = crate::meeting::MeetingContext::load(&dir)?;
    fs::write(&md, crate::transcribe::render_markdown(&dir, &meta, meeting.as_ref(), &model, &segs))?;
    for (id, name) in &names {
        println!("speaker {id}: {name}");
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
    fn segments_get_the_speaker_they_overlap_most_numbered_by_first_turn() {
        let mut s = vec![seg("remote", 0.0, 3.0), seg("me", 3.0, 4.0), seg("remote", 4.0, 8.0), seg("remote", 8.5, 9.0), seg("remote", 20.0, 21.0)];
        // The diarizer's own ids: 5 speaks first, then 2; a segment mostly in 5's turn is 5's.
        let turns = [(0.0, 3.2, 5), (3.9, 8.6, 2), (8.6, 9.0, 5)];
        assert_eq!(assign(&mut s, &turns), 2);
        let ids: Vec<_> = s.iter().map(|x| x.speaker_id).collect();
        assert_eq!(ids, [Some(1), None, Some(2), Some(1), None]);
    }

    #[test]
    fn a_sentence_two_people_share_is_split_at_the_turn() {
        let mut one = seg("remote", 0.0, 4.0);
        one.text = "Hello there. Hi, how are you doing".into();
        one.words = vec![[0.0, 0.4], [0.5, 1.0], [2.0, 2.3], [2.4, 2.6], [2.7, 2.9], [3.0, 3.2], [3.3, 4.0]];
        let turns = [(0.0, 1.2, 7), (1.9, 4.1, 3), (2.45, 2.55, 7)]; // a blip of 7 inside 3's turn
        let mut s = vec![one];
        assert_eq!(assign(&mut s, &turns), 2);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!((s[0].text.as_str(), s[0].speaker_id, s[0].end), ("Hello there.", Some(1), 1.0));
        assert_eq!((s[1].text.as_str(), s[1].speaker_id, s[1].start), ("Hi, how are you doing", Some(2), 2.0));
    }

    #[test]
    fn without_the_speaker_model_teams_names_and_splits_the_remote_words() {
        let dir = std::env::temp_dir().join(format!("echo-speakers-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // Dana's ring 0.5–8.5 (lit late by 0.5 s), Lee's 8.5–20.5; the user's tile 20.5–40.5.
        let line = |t: f64, k: &str, n: &str, add: &[&str], del: &[&str], me: bool| {
            let mut v = json!({ "t": t, "k": k, "n": n, "add": add, "del": del });
            if me {
                v["me"] = true.into();
            }
            v.to_string()
        };
        let log = [
            line(0.0, "tile:Dana", "Dana Lin", &["tile"], &[], false),
            line(0.0, "tile:Lee", "Lee Park", &["tile"], &[], false),
            line(0.0, "tile:Jane", "Jane Doe", &["tile"], &[], true),
            line(0.5, "tile:Dana", "Dana Lin", &["ring"], &[], false),
            line(8.5, "tile:Dana", "Dana Lin", &[], &["ring"], false),
            line(8.5, "tile:Lee", "Lee Park", &["ring"], &[], false),
            line(20.5, "tile:Lee", "Lee Park", &[], &["ring"], false),
            line(20.5, "tile:Jane", "Jane Doe", &["ring"], &[], true),
            line(40.5, "tile:Jane", "Jane Doe", &[], &["ring"], true),
        ];
        fs::write(dir.join(crate::speaking::FILE), log.join("\n")).unwrap();
        let mut a = seg("remote", 0.0, 12.0);
        a.text = "So that is the plan. Sounds good to me".into();
        a.words = vec![[0.5, 1.0], [1.2, 1.5], [1.6, 1.8], [1.9, 2.1], [2.2, 7.9], [8.6, 9.0], [9.1, 9.5], [9.6, 9.8], [9.9, 12.0]];
        let mut b = seg("remote", 13.0, 20.0);
        b.text = "Let us ship it".into();
        let mut segs = vec![a, b, seg("me", 20.0, 40.0)];
        let tl = crate::speaking::Timeline::learn(&dir, &segs, None).expect("calibrated");
        split_by_timeline(&mut segs, &tl);
        let got: Vec<(&str, Option<&str>)> = segs.iter().map(|s| (s.text.as_str(), s.name.as_deref())).collect();
        assert_eq!(got, [
            ("So that is the plan.", Some("Dana Lin")),
            ("Sounds good to me", Some("Lee Park")),
            ("Let us ship it", Some("Lee Park")),
            ("x", None),
        ]);
        // With voices from the speaker model, a voice takes the name its words were lit for.
        let mut v = vec![seg("remote", 0.5, 8.0), seg("remote", 9.0, 20.0)];
        v[0].text = "one two three four".into();
        v[1].text = "five six seven eight".into();
        v[0].speaker_id = Some(1);
        v[1].speaker_id = Some(2);
        let voted = vote(&v, &tl);
        assert_eq!(voted.get(&1).map(String::as_str), Some("Dana Lin"));
        assert_eq!(voted.get(&2).map(String::as_str), Some("Lee Park"));
        let _ = fs::remove_dir_all(&dir);
    }
}
