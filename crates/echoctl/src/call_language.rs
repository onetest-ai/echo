//! A call's one language (#234): set for it, the only one allowed, or detected once from its first
//! utterances and then kept for the whole call, both tracks (`transcribe::Transcriber` holds one).
//! Detection is a second encoder run per utterance, so it stops once the language is decided.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;

use crate::transcribe::Decoded;

/// Below this, language detection is unreliable: the utterance doesn't vote on the call's language.
const SHORT_UTTERANCE_S: f64 = 1.5;
/// The call's language is decided once this many utterances have voted with this much speech
/// between them (each vote weighted by its seconds, so a short greeting in another language
/// doesn't outweigh the call)…
const DECIDE_VOTES: usize = 3;
const DECIDE_SPEECH_S: f64 = 10.0;
/// …or after this many votes, however short (a call of short replies).
const MAX_VOTES: usize = 6;
/// A call whose detections never vote (no allowed language heard, nothing written) stops detecting after this many.
const MAX_DETECTIONS: usize = 12;

/// An utterance's detection: allowed languages with their probabilities, best first.
pub(crate) type Ranked = Vec<(i32, f32)>;
/// The language to decode an utterance in, given its detection (`CallLanguage::pick`).
pub(crate) type Pick<'a> = dyn Fn(&[(i32, f32)]) -> Option<i32> + 'a;

/// The one language of a call (#234): fixed from the start (set in its `meta.json`, or the only
/// one allowed), or decided from the detections of its first utterances, on either track, each
/// weighted by its seconds of speech; then fixed for the rest of the call. Never checked again:
/// a call has one language. Never chosen by comparing decode log-probabilities across
/// languages — translations and hallucinations score higher than faithful transcriptions
/// (tested, 2026-10-09).
#[derive(Debug, Default)]
pub(crate) struct CallLanguage {
    pub(crate) fixed: Option<i32>,
    /// Each language's probability summed over the votes, weighted by their seconds.
    tally: Vec<(i32, f64)>,
    pub(crate) votes: usize,
    pub(crate) speech_s: f64,
    /// A short utterance's pick before any vote: what to decode with until a vote comes.
    short: Option<i32>,
    /// Detections so far, voting or not (`MAX_DETECTIONS`).
    detections: usize,
    /// The allowed list's first: the call's language when its detections never vote.
    fallback: Option<i32>,
}

/// How to decode one utterance: in this language, or detecting it among the allowed ones.
#[derive(Debug, PartialEq)]
enum Plan {
    Use(i32),
    Detect,
}

impl CallLanguage {
    pub(crate) fn new(fixed: Option<i32>, allowed: &[i32]) -> CallLanguage {
        CallLanguage { fixed, fallback: allowed.first().copied(), ..Default::default() }
    }

    fn plan(&self, dur: f64) -> Plan {
        match (self.fixed, self.leader()) {
            (Some(l), _) => Plan::Use(l),
            (None, Some(l)) if dur < SHORT_UTTERANCE_S => Plan::Use(l),
            _ => Plan::Detect,
        }
    }

    /// The language with the most weighted votes; before any, a short utterance's pick.
    fn leader(&self) -> Option<i32> {
        self.fixed.or_else(|| self.tally.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|t| t.0)).or(self.short)
    }

    /// The language to decode a `dur`-second utterance with, `ranked` being its own detection:
    /// the leader of the votes so far and its own.
    fn pick(&self, ranked: &[(i32, f32)], dur: f64) -> Option<i32> {
        if self.fixed.is_some() || dur < SHORT_UTTERANCE_S {
            return self.leader().or(ranked.first().map(|r| r.0));
        }
        let mut c = CallLanguage { tally: self.tally.clone(), ..Default::default() };
        c.add(ranked, dur);
        c.leader()
    }

    fn add(&mut self, ranked: &[(i32, f32)], dur: f64) {
        for &(id, p) in ranked {
            match self.tally.iter_mut().find(|t| t.0 == id) {
                Some(t) => t.1 += p as f64 * dur,
                None => self.tally.push((id, p as f64 * dur)),
            }
        }
        self.votes += 1;
        self.speech_s += dur;
    }

    /// Count an utterance's detection; `spoken`: its decode wrote something (hold music, a keyboard or a
    /// hallucination dropped as such doesn't vote). Returns the language when this detection decided it: on the
    /// votes (`DECIDE_VOTES`, `MAX_VOTES`), or after `MAX_DETECTIONS` on what there is, else the allowed list's first.
    pub(crate) fn vote(&mut self, ranked: &[(i32, f32)], dur: f64, spoken: bool) -> Option<i32> {
        if self.fixed.is_some() {
            return None;
        }
        self.detections += 1;
        if spoken && !ranked.is_empty() {
            if dur < SHORT_UTTERANCE_S {
                self.short = self.short.or(Some(ranked[0].0));
            } else {
                self.add(ranked, dur);
                if (self.votes >= DECIDE_VOTES && self.speech_s >= DECIDE_SPEECH_S) || self.votes >= MAX_VOTES {
                    return self.settle();
                }
            }
        }
        if self.detections >= MAX_DETECTIONS {
            self.fixed = self.leader().or(self.fallback);
            return self.fixed;
        }
        None
    }

    /// Decide now on the votes so far (the call ended first). Returns the language when it was decided here.
    pub(crate) fn settle(&mut self) -> Option<i32> {
        if self.fixed.is_some() || self.votes == 0 {
            return None;
        }
        self.fixed = self.leader();
        self.fixed
    }
}

/// One utterance of `dur` seconds in the call's language: `decode(allowed, pick)` decodes it in the one allowed language,
/// or detects among several and decodes in the language `pick` gives for its detection. Before the language is decided,
/// a server's own pick that isn't the call's leader is decoded again in the leader. Returns the language, the
/// sentences, and the language when this utterance decided it.
pub(crate) fn with_language(
    call: &Mutex<CallLanguage>,
    allowed: &[i32],
    dur: f64,
    mut decode: impl FnMut(&[i32], &Pick<'_>) -> Result<(Option<Ranked>, String, Decoded)>,
) -> Result<(String, Decoded, Option<i32>)> {
    let lock = || call.lock().unwrap_or_else(|e| e.into_inner());
    let pick = |r: &[(i32, f32)]| lock().pick(r, dur);
    let plan = lock().plan(dur);
    let (ranked, mut lang, mut segs) = match plan {
        Plan::Use(l) => decode(&[l], &pick)?,
        Plan::Detect => decode(allowed, &pick)?,
    };
    let Some(ranked) = ranked else { return Ok((lang, segs, None)) };
    let decided = lock().vote(&ranked, dur, !segs.is_empty());
    if segs.is_empty() {
        return Ok((lang, segs, decided));
    }
    let leader = lock().leader();
    if let Some(l) = leader.filter(|l| whisper_rs::get_lang_str(*l) != Some(lang.as_str())) {
        (_, lang, segs) = decode(&[l], &pick)?;
    }
    Ok((lang, segs, decided))
}

/// A call's language before it starts: the one set for it while it is allowed (or any is), else the only one
/// allowed; None: detect it. A set language the allowed list no longer has is decided again (the languages were
/// changed to correct a wrong one).
pub(crate) fn fixed_language(set: Option<i32>, allowed: &[i32]) -> Option<i32> {
    let set = set.filter(|l| allowed.is_empty() || allowed.contains(l));
    match allowed {
        [only] => set.or(Some(*only)),
        _ => set,
    }
}

/// The language set in a call's `meta.json` (`"language": "uk"`: decided by an earlier pass, or set for the call).
pub(crate) fn meta_language(dir: &Path) -> Option<i32> {
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).ok()?).ok()?;
    whisper_rs::get_lang_id(meta["language"].as_str()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (i32, i32, i32) {
        let id = |c| whisper_rs::get_lang_id(c).unwrap();
        (id("en"), id("uk"), id("ru"))
    }

    /// A call of utterances (seconds, what a detection would rank) through `with_language`: each decode's allowed
    /// languages and the language it was decoded in. `own_pick`: the engine ignores `pick` and decodes in its own
    /// top language, as a transcription server does.
    fn run_call(fixed: Option<i32>, allowed: &[i32], call: &[(f64, Ranked)], own_pick: bool) -> (Vec<(Vec<i32>, String)>, Mutex<CallLanguage>) {
        let spoken: Vec<(f64, Ranked, bool)> = call.iter().map(|(d, r)| (*d, r.clone(), true)).collect();
        run_spoken(fixed, allowed, &spoken, own_pick)
    }

    /// As `run_call`, each utterance also saying whether its decode had any sentences.
    fn run_spoken(fixed: Option<i32>, allowed: &[i32], call: &[(f64, Ranked, bool)], own_pick: bool) -> (Vec<(Vec<i32>, String)>, Mutex<CallLanguage>) {
        let lang = Mutex::new(CallLanguage::new(fixed, allowed));
        let mut decodes = vec![];
        for (dur, ranked, spoken) in call {
            with_language(&lang, allowed, *dur, |allowed, pick| {
                let (r, id) = match allowed {
                    [only] => (None, Some(*only)),
                    _ => (Some(ranked.clone()), if own_pick { ranked.first().map(|x| x.0) } else { pick(ranked) }),
                };
                let code = id.and_then(whisper_rs::get_lang_str).unwrap_or("auto").to_string();
                decodes.push((allowed.to_vec(), code.clone()));
                let segs = if *spoken { vec![crate::transcribe::Piece::new(0.0, 1.0, "words".into(), vec![])] } else { vec![] };
                Ok((r, code, segs))
            })
            .unwrap();
        }
        (decodes, lang)
    }

    fn detections(decodes: &[(Vec<i32>, String)]) -> usize {
        decodes.iter().filter(|(a, _)| a.len() != 1).count()
    }

    #[test]
    fn a_call_set_to_a_language_or_allowing_one_never_detects() {
        let (en, uk, ru) = ids();
        assert_eq!(fixed_language(None, &[en]), Some(en), "the only language allowed");
        assert_eq!(fixed_language(Some(uk), &[en, uk, ru]), Some(uk), "meta.json's language");
        assert_eq!(fixed_language(Some(uk), &[]), Some(uk), "meta.json's language, any allowed");
        assert_eq!(fixed_language(None, &[en, uk, ru]), None);
        assert_eq!(fixed_language(None, &[]), None, "any language: detect");
        let call: Vec<(f64, Ranked)> = (0..8).map(|i| (1.0 + i as f64, vec![(ru, 0.9), (uk, 0.1)])).collect();
        let (decodes, _) = run_call(Some(uk), &[en, uk, ru], &call, false);
        assert!(decodes.iter().all(|(a, l)| a == &[uk] && l == "uk"), "{decodes:?}");
    }

    #[test]
    fn a_stored_language_no_longer_allowed_is_decided_again() {
        let (en, uk, ru) = ids();
        // A Ukrainian call decided as Russian, then the languages set to en, uk: detect again.
        assert_eq!(fixed_language(Some(ru), &[en, uk]), None);
        // …to en only: the one allowed.
        assert_eq!(fixed_language(Some(ru), &[en]), Some(en));
        assert_eq!(fixed_language(Some(uk), &[en]), Some(en));
    }

    #[test]
    fn an_utterance_with_no_sentences_does_not_vote() {
        let (en, uk, ru) = ids();
        let music: Ranked = vec![(ru, 0.9), (uk, 0.05), (en, 0.05)];
        let speech: Ranked = vec![(en, 0.95), (uk, 0.03), (ru, 0.02)];
        // Hold music detected as Russian four times, 4 s each, nothing written; then English speech.
        let mut call: Vec<(f64, Ranked, bool)> = (0..4).map(|_| (4.0, music.clone(), false)).collect();
        call.extend((0..3).map(|_| (4.0, speech.clone(), true)));
        let (_, lang) = run_spoken(None, &[en, uk, ru], &call, false);
        let c = lang.lock().unwrap();
        assert_eq!((c.fixed, c.votes), (Some(en), 3), "only the speech voted");
    }

    #[test]
    fn detection_stops_after_max_detections_even_when_nothing_votes() {
        let (en, uk, ru) = ids();
        // Nemotron hearing no allowed language (an empty vote) on every utterance.
        let call: Vec<(f64, Ranked)> = (0..30).map(|_| (4.0, vec![])).collect();
        let (decodes, lang) = run_call(None, &[uk, en, ru], &call, false);
        assert_eq!(detections(&decodes), MAX_DETECTIONS, "{decodes:?}");
        assert_eq!(lang.lock().unwrap().fixed, Some(uk), "the allowed list's first");
        assert!(decodes[MAX_DETECTIONS..].iter().all(|(a, l)| a == &[uk] && l == "uk"));
        // Noise only, every detection without sentences: also capped, on the list's first.
        let noise: Vec<(f64, Ranked, bool)> = (0..30).map(|_| (4.0, vec![(ru, 0.9)], false)).collect();
        let (decodes, lang) = run_spoken(None, &[en, uk, ru], &noise, false);
        assert_eq!(detections(&decodes), MAX_DETECTIONS);
        assert_eq!(lang.lock().unwrap().fixed, Some(en));
        // Votes before the cap: their leader.
        let mut some: Vec<(f64, Ranked)> = vec![(4.0, vec![(uk, 0.9), (en, 0.1)])];
        some.extend((0..30).map(|_| (4.0, vec![])));
        let (decodes, lang) = run_call(None, &[en, uk, ru], &some, false);
        assert_eq!(detections(&decodes), MAX_DETECTIONS);
        assert_eq!(lang.lock().unwrap().fixed, Some(uk));
    }

    #[test]
    fn meta_language_is_read_from_the_call() {
        let (_env, _) = crate::paths::testenv::folders("transcribe-meta-language");
        let dir = crate::paths::home().join("calls/2026-11-09_100000-x");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(meta_language(&dir), None, "no meta.json");
        fs::write(dir.join("meta.json"), r#"{"title": "x", "languages": ["en", "uk"]}"#).unwrap();
        assert_eq!(meta_language(&dir), None);
        fs::write(dir.join("meta.json"), r#"{"title": "x", "language": "uk"}"#).unwrap();
        assert_eq!(meta_language(&dir), Some(ids().1));
        fs::write(dir.join("meta.json"), r#"{"language": "klingon"}"#).unwrap();
        assert_eq!(meta_language(&dir), None, "not a language");
    }

    #[test]
    fn detection_runs_on_the_first_utterances_only_then_every_one_is_decoded_in_the_call_language() {
        let (en, uk, ru) = ids();
        let english: Ranked = vec![(en, 0.9), (uk, 0.06), (ru, 0.04)];
        // Both tracks' utterances in order: a short one first, then long and short ones.
        let call: Vec<(f64, Ranked)> = [0.8, 4.0, 1.0, 3.5, 6.0, 2.0, 5.0, 0.9, 8.0, 3.0, 2.5, 7.0]
            .iter()
            .map(|d| (*d, english.clone()))
            .collect();
        let (decodes, lang) = run_call(None, &[en, uk, ru], &call, false);
        let detections = decodes.iter().filter(|(a, _)| a.len() != 1).count();
        // The short first one (no vote yet), then 4.0, 3.5 and 6.0 s: 13.5 s in 3 votes decide.
        assert_eq!(detections, 4, "{decodes:?}");
        assert!(detections <= MAX_VOTES + 1);
        assert_eq!(lang.lock().unwrap().fixed, Some(en));
        assert!(decodes[5..].iter().all(|(a, l)| a == &[en] && l == "en"), "{decodes:?}");
        assert!(decodes.iter().all(|(_, l)| l == "en"));
        // The short one between votes used the leader without detecting.
        assert_eq!(decodes[2], (vec![en], "en".to_string()));
    }

    #[test]
    fn a_short_greeting_in_another_language_does_not_decide_the_call() {
        let (en, uk, ru) = ids();
        let call = vec![
            (1.6, vec![(ru, 0.92), (uk, 0.05), (en, 0.03)]), // "Привет!"
            (5.0, vec![(en, 0.95), (ru, 0.03), (uk, 0.02)]),
            (5.5, vec![(en, 0.93), (ru, 0.04), (uk, 0.03)]),
            (4.0, vec![(en, 0.90), (ru, 0.06), (uk, 0.04)]),
        ];
        let (decodes, lang) = run_call(None, &[en, uk, ru], &call, false);
        let c = lang.lock().unwrap();
        assert_eq!(c.fixed, Some(en), "decided at the third vote: 12.1 s");
        assert_eq!(c.votes, 3);
        assert_eq!(decodes[0].1, "ru", "the greeting itself is decoded as heard");
        assert_eq!(decodes[3], (vec![en], "en".to_string()), "no detection after the decision");
    }

    #[test]
    fn ukrainian_and_russian_calls_are_kept_apart() {
        let (en, uk, ru) = ids();
        // large-v3 on Ukrainian: close to Russian on each utterance, but ahead on the sum.
        let uk_call: Vec<(f64, Ranked)> = (0..4).map(|i| (4.0, if i == 1 { vec![(ru, 0.5), (uk, 0.45), (en, 0.05)] } else { vec![(uk, 0.55), (ru, 0.4), (en, 0.05)] })).collect();
        let (decodes, lang) = run_call(None, &[en, uk, ru], &uk_call, false);
        assert_eq!(lang.lock().unwrap().fixed, Some(uk));
        assert_eq!(decodes[1].1, "uk", "a vote for Russian doesn't switch the leader");
        let ru_call: Vec<(f64, Ranked)> = (0..4).map(|_| (4.0, vec![(ru, 0.7), (uk, 0.25), (en, 0.05)])).collect();
        let (decodes, lang) = run_call(None, &[en, uk, ru], &ru_call, false);
        assert_eq!(lang.lock().unwrap().fixed, Some(ru));
        assert!(decodes.iter().all(|(_, l)| l == "ru"));
    }

    #[test]
    fn short_replies_decide_after_max_votes_and_an_early_end_settles_on_the_votes() {
        let (en, uk, ru) = ids();
        let call: Vec<(f64, Ranked)> = (0..10).map(|_| (1.6, vec![(uk, 0.8), (ru, 0.2)])).collect();
        let (decodes, lang) = run_call(None, &[en, uk, ru], &call, false);
        assert_eq!(decodes.iter().filter(|(a, _)| a.len() != 1).count(), MAX_VOTES);
        assert_eq!(lang.lock().unwrap().fixed, Some(uk));
        let (_, lang) = run_call(None, &[en, uk, ru], &call[..1], false);
        let mut c = lang.into_inner().unwrap();
        assert_eq!(c.fixed, None);
        assert_eq!(c.settle(), Some(uk), "one vote, the call over: it decides");
        assert_eq!(c.settle(), None, "decided once");
        assert_eq!(CallLanguage::new(None, &[]).settle(), None, "no votes: nothing to decide");
    }

    #[test]
    fn a_servers_own_pick_is_decoded_again_in_the_calls_leader() {
        let (en, uk, ru) = ids();
        let call = vec![
            (5.0, vec![(en, 0.9), (uk, 0.1)]),
            (2.0, vec![(uk, 0.6), (en, 0.4)]), // the server decodes it in uk; the call leads with en
        ];
        let (decodes, _) = run_call(None, &[en, uk, ru], &call, true);
        assert_eq!(decodes[1..], [(vec![en, uk, ru], "uk".to_string()), (vec![en], "en".to_string())]);
    }
}
