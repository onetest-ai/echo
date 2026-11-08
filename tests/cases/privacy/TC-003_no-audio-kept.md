---
id: TC-003
title: No audio is kept once a call is transcribed
priority: critical
type: functional
requirements: [F-PRIV-03]
module: privacy
run: agent
platform: macos
tags: [privacy, audio, test-call, recordings]
external_id: TC-PRIV-003, TC-TRN-002
---

# TC-003: No audio is kept once a call is transcribed

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"). For `acme-sync` it listed the call's folder while `echoctl capture` recorded (`during.ls`) and after `echoctl transcribe` with the default settings (`keep_audio` off, no `--keep-audio`: `after.ls`)
- In S-17, stage 2 (`recording/TC-001`) recorded a call through the app's watcher, which transcribed it itself; standalone, run `recording/TC-001` first
- Every command after `. <dir>/env` on the run's data (`make_data.py <dir> --ai fake`); `P="$(python3 tests/fixtures/recordings.py path)/acme-sync"`
- `keep_audio` is off (the default of the test data): `target/debug/echoctl config` prints `"keep_audio": false`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `cat "$P/during.ls"` | While the call recorded, the folder held `mic.wav` and `remote.wav` (audio exists while recording) |
| 2 | `cat "$P/after.ls"; grep -o '"audio": *"[^"]*"' "$P/call"/*/meta.json` | After the transcription: `meta.json`, `segments.json`, `transcript.md` (and `live.jsonl`, `expected.json`), no `.wav`; `meta.json` says the audio was deleted after transcription |
| 3 | `ls "$ECHOCTL_HOME"/calls/*-rec-001-next-call; find "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR" -name '*.wav'` | The call the app recorded and transcribed by itself has `transcript.md` and no `.wav`; `find` prints nothing: not one `.wav` under the data folder or the config folder |

## Expected Final State
Once transcribed, only text stays: by `echoctl transcribe` and by the app after a call, not one `.wav` under the data folder or the config folder.

## Teardown
- Nothing to undo

## Notes
- Before 2026-11-08 this case recorded and transcribed a call of its own; the prepared recording is that same capture and transcription, made once per build.
- `transcribe --keep-audio` keeps the files on purpose; no case of S-17 uses it now, so the `find` needs no `-newer`.
