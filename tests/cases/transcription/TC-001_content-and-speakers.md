---
id: TC-001
title: The transcript has the words, speakers and order of the call
priority: critical
type: functional
requirements: [F-TR-01, F-STT-04, F-STT-03]
module: transcription
run: agent
platform: macos
tags: [transcription, recordings]
external_id: TC-TRN-001
---

# TC-001: The transcript has the words, speakers and order of the call

**Module:** Transcription | **Priority:** Critical | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`, once per run: `app_profile.md` → "Recorded calls"): the `acme-sync` call was played through TestCall, captured as two tracks and transcribed once by the build under test; this case scores that transcript and transcribes nothing
- Test data built by `tests/fixtures/make-data.sh <dir>`; every command after `. <dir>/env`

## Test Data

| Field | Value |
|-------|-------|
| Recording | `acme-sync`: 6 turns, 36 s, Me + Dana (English), title "TRN-001 acme" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `CALL="$(python3 tests/fixtures/recordings.py copy acme-sync --title "TRN-001 acme")"; ls "$CALL"` | It prints the new folder under `$ECHOCTL_HOME/calls`; the folder has `transcript.md`, `segments.json`, `expected.json` and `meta.json`, no `.wav` |
| 2 | `python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true` (the 15% word-error limit is on the whole call: the top-level `wer` is at most `0.15`, not each turn's); every turn `speaker_ok` and `lang_ok` true; `transcribed_in_silence` `[]` |
| 3 | Read `"$CALL/transcript.md"` | Turns in time order, the user's as Me and Dana's as Remote; "Berlin first" and "SSO checklist" spelled as said; the header names the model (`large-v3-turbo-q5_0`) |

## Expected Final State
The six turns of the Acme call, as captured and transcribed by the build, come out with at most 15% word errors over the whole call, each with the right speaker and language, in order.

## Notes
- This is the one check of the words of a captured call. The recording cases check capturing (tracks, start, stop) and do not score words again.
- Before 2026-11-08 this case built the call from the clips (no capture) and transcribed it itself; the prepared recording is the same conversation through the real capture path (`recording/TC-014`), transcribed once per build.
