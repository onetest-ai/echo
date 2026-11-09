---
id: TC-002
title: With English, Ukrainian and Russian allowed, a call in Ukrainian comes out in Ukrainian and a call in Russian in Russian
priority: high
type: functional
requirements: [F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription, languages, recordings]
external_id: TC-TRN-003
---

# TC-002: A call comes out in its one language

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `ukrainian-call` and `russian-call` were captured and transcribed once with the workspace languages en, uk, ru and no pin of their own
- Test data built by `tests/fixtures/make-data.sh <dir>`; every command after `. <dir>/env`; `P="$(python3 tests/fixtures/recordings.py path)"`

## Test Data

| Field | Value |
|-------|-------|
| Recording | `ukrainian-call`: 3 turns, Ukrainian only (two voices), title "TRN-002 Ukrainian" |
| Recording | `russian-call`: 2 turns, Russian only, title "TRN-002 Russian" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep -m1 "(languages:" "$P/ukrainian-call/transcribe.out"; grep -h "  language:" "$P/ukrainian-call/capture.err" "$P/ukrainian-call/transcribe.out"` | "(languages: en, uk, ru)"; one line "language: uk (from … s of speech in … utterances)" from the pass that decided it (the live pass during the capture, else the transcription) and, when the capture decided it, "language: uk (set for this call)" from the transcription: detected once per call, never per sentence, and not again |
| 2 | `CALL="$(python3 tests/fixtures/recordings.py copy ukrainian-call --title "TRN-002 Ukrainian")"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true`; top-level `wer` at most 0.15; every turn `lang_ok` (uk); no turn translated |
| 3 | `grep '"language"' "$CALL/meta.json"` | `"language": "uk"`: kept for a retranscription |
| 4 | `grep -h "  language:" "$P/russian-call/capture.err" "$P/russian-call/transcribe.out"; CALL="$(python3 tests/fixtures/recordings.py copy russian-call --title "TRN-002 Russian")"; python3 tests/fixtures/audio/make_audio.py score "$CALL"; grep '"language"' "$CALL/meta.json"` | "language: ru (…)"; exit code 0, `"pass": true`, `wer` at most 0.15, both turns `lang_ok` (ru); `"language": "ru"` |

## Expected Final State
With three languages allowed, each call is transcribed in the one language it is in (Ukrainian and Russian kept apart), detected once from its first sentences and kept in its `meta.json`, at most 15% word errors.

## Notes
- Before #234 this case was "Each sentence stays in the language it was spoken in" on `mixed-languages`: a call now has one language, detected once (calls in several languages were dropped).
