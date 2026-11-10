---
id: TC-003
title: A call started without a language is transcribed in the default one, Settings' first
priority: high
type: functional
requirements: [F-STT-02, F-SET-07]
module: transcription
run: agent
platform: macos
tags: [transcription, languages, recordings]
external_id: TC-TRN-004
auto: [tests/auto/specs/S-14.spec.mjs]
---

# TC-003: A call started without a language is in the default one

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `two-remotes` was captured and transcribed once with the workspace languages set to English (`config --languages en`) and no language of its own
- Test data built by `tests/fixtures/make-data.sh <dir>`; every command after `. <dir>/env`

## Test Data

| Field | Value |
|-------|-------|
| Recording | `two-remotes`: 6 turns, Me + Dana + Omar on one remote track, title "TRN-003 en only" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep '"language"' "$(python3 tests/fixtures/recordings.py path)/two-remotes/meta.json"` | `"language": "en"`: the default, written when the recording started |
| 2 | `CALL="$(python3 tests/fixtures/recordings.py copy two-remotes --title "TRN-003 en only")"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0 and `"pass": true`; top-level `wer` at most 0.15 |
| 3 | `python3 -c "import json,sys;print({s['lang'] for s in json.load(open(sys.argv[1]+'/segments.json'))})" "$CALL"` | `{'en'}` |
| 4 | `grep "  language:" "$(python3 tests/fixtures/recordings.py path)/two-remotes/transcribe.out"` | "language: en", with no "(from … utterances)": nothing detected it |

## Expected Final State
A call started without a language is in the default one (Settings' first, English): every segment of a two-voice remote call is English, nothing detected, at most 15% word errors over the whole call.
