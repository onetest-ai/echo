---
id: TC-008
title: On speakers, the other side isn't duplicated as Me
priority: high
type: functional
requirements: [F-STT-03, F-STT-07]
module: recording
run: person
platform: macos
tags: [recording, echo]
external_id: TC-REC-008
---

# TC-008: On speakers, the other side isn't duplicated as Me

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 2 minutes

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a call on the laptop speakers, the other person talking a lot; a minute in, say a short "yes" while they talk, and once talk over them for a sentence | — |
| 2 | Read the transcript | Their words are not repeated as Me lines; your "yes" and the sentence you said over them are there as Me |
| 3 | `grep -E '^  echo' <data folder>/.echo/app.log` | "echo found N s into the call: D ms, gain G, r R …" (N about 30–90 s after they started talking) and "echo: D ms, gain G, r R; skipped S s of the mic", S above 0; no "echo cancellation: on" after them (the mic isn't decoded a second time) |

## Expected Final State
The speakers' sound is kept out of your track: found during the call and not transcribed, while your own words are.

## Notes
- Stays person-run: it needs a real microphone hearing real speakers. The test call's mic is BlackHole (no acoustic path), so TestCall shows no echo to cancel; its remote track is checked for wrong speakers by `recording/TC-014` and `TC-018`. The same behaviour on a made-up echo (fed from files, double talk included) is `transcription/TC-010`.
- What a real room changes (#238): the stretches skipped are those whose loudness follows the remote's, gain × it, within about 1.6 dB frame by frame. A room that colours the sound more than that skips less (the mic is then transcribed, and the transcript's echo filter drops the lines repeating the other side). If step 3 shows the echo found but "skipped 0.0 s", the echo cancellation decodes the mic again at the end, as before: note G, R and how the transcript reads.
