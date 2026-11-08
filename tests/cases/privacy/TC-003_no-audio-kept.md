---
id: TC-003
title: No audio is kept once a call is transcribed
priority: critical
type: functional
requirements: [F-PRIV-03]
module: privacy
run: agent
platform: macos
tags: [privacy, audio, second-instance, recordings]
external_id: TC-PRIV-003, TC-TRN-002
---

# TC-003: No audio is kept once a call is transcribed

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- **The main call**, recorded by the app's watcher from files and transcribed by the app itself: `acme-sync` as "REC-001 next call", its folder listed while it recorded into `$D/rec-001.during`. In S-17: the scenario's prelude (recording/TC-001 steps 1–4), hung up in window/TC-014. On its own: `recording/TC-001` steps 1–6 first
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"). For `acme-sync` it listed the call's folder while `echoctl capture` recorded (`during.ls`) and after `echoctl transcribe` with the default settings (`keep_audio` off, no `--keep-audio`: `after.ls`)
- Every command after the prelude of `recording/TC-001` on the run's data (`make_data.py <dir> --ai fake`); `P="$(python3 tests/fixtures/recordings.py path)/acme-sync"`
- `keep_audio` is off (the default of the test data): `target/debug/echoctl config` prints `"keep_audio": false`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep -E "\.wav" "$D/rec-001.during"; cat "$P/during.ls"` | While each call recorded, its folder held `mic.wav` and `remote.wav` (audio exists while recording): the main call's listings and the capture's |
| 2 | `cat "$P/after.ls"; grep -o '"audio": *"[^"]*"' "$P/call"/*/meta.json` | After `echoctl transcribe`: `meta.json`, `segments.json`, `transcript.md` (and `live.jsonl`, `expected.json`), no `.wav`; `meta.json` says the audio was deleted after transcription |
| 3 | `ls "$ECHOCTL_HOME"/calls/*-rec-001-next-call; grep -o '"audio": *"[^"]*"' "$ECHOCTL_HOME"/calls/*-rec-001-next-call/meta.json; find "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR" -name '*.wav'` | The call the app recorded and transcribed by itself has `transcript.md` and no `.wav`, and its `meta.json` says the audio was deleted after transcription; `find` prints nothing: not one `.wav` under the data folder or the config folder (the feed's files are in `$D/feed` and `$D/conv`, outside both) |

## Expected Final State
Once transcribed, only text stays: by `echoctl transcribe` and by the app after a call, not one `.wav` under the data folder or the config folder.

## Teardown
- Nothing to undo

## Notes
- Before 2026-11-08 this case recorded and transcribed a call of its own; now the scenario's main call (the app's path) and the prepared recording (`echoctl transcribe`'s path) are the evidence.
- `transcribe --keep-audio` keeps the files on purpose; no case of S-17 uses it, and no stage of S-17 keeps tracks any more (window/TC-013 hangs off the main call since 2026-11-08 evening), so the `find` needs no `-newer`.
- Run 2026-11-08 evening (S-17, CLI part): PASS — the main call's listings had both wavs, after: none, `"audio": "deleted after transcription"`, `find` empty.
