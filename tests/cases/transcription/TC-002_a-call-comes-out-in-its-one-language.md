---
id: TC-002
title: A call recorded in Ukrainian comes out in Ukrainian and one recorded in Russian in Russian, nothing detected
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
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `ukrainian-call` and `russian-call` were captured and transcribed once with the workspace languages en, uk, ru, each started in its own language (`capture --languages uk` / `ru`, as the Record button's choice)
- Test data built by `tests/fixtures/make-data.sh <dir>`; every command after `. <dir>/env`; `P="$(python3 tests/fixtures/recordings.py path)"`

## Test Data

| Field | Value |
|-------|-------|
| Recording | `ukrainian-call`: 3 turns, Ukrainian only (two voices), title "TRN-002 Ukrainian" |
| Recording | `russian-call`: 2 turns, Russian only, title "TRN-002 Russian" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep -h "  language:" "$P/ukrainian-call/capture.err" "$P/ukrainian-call/transcribe.out"` | Only "language: uk" lines (the live pass and the transcription): the language it was started in, no "(from … utterances)" — nothing detected it |
| 2 | `CALL="$(python3 tests/fixtures/recordings.py copy ukrainian-call --title "TRN-002 Ukrainian")"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true`; top-level `wer` at most 0.15; every turn `lang_ok` (uk); no turn translated |
| 3 | `grep '"language"' "$CALL/meta.json"` | `"language": "uk"`: kept for a retranscription |
| 4 | `grep -h "  language:" "$P/russian-call/capture.err" "$P/russian-call/transcribe.out"; CALL="$(python3 tests/fixtures/recordings.py copy russian-call --title "TRN-002 Russian")"; python3 tests/fixtures/audio/make_audio.py score "$CALL"; grep '"language"' "$CALL/meta.json"` | Only "language: ru" lines; exit code 0, `"pass": true`, `wer` at most 0.15, both turns `lang_ok` (ru); `"language": "ru"` |

## Expected Final State
Each call is transcribed in the language it was started in (Ukrainian and Russian kept apart), kept in its `meta.json`, at most 15% word errors; nothing detects a call's language.

## Notes
- Until 2026-11-09 the language was detected once from the first sentences (#234); now it is chosen when the call starts.
- Before #234 this case was "Each sentence stays in the language it was spoken in" on `mixed-languages`: a call now has one language, detected once (calls in several languages were dropped).
