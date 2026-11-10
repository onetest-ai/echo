---
id: TC-014
title: A call is recorded as two tracks, written while it records, aligned and closed at its end, and transcribed from both
priority: critical
type: functional
requirements: [F-REC-07, F-STT-01, F-STT-04, F-PRIV-03]
module: recording
run: agent
platform: macos
tags: [recording, tracks, second-instance, recordings]
auto: [tests/auto/specs/S-17.spec.mjs]
---

# TC-014: A call is recorded as two tracks, written while it records, aligned and closed at its end, and transcribed from both

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- **The main call**, recorded by the app's watcher from files (`app_profile.md` → "Recording with file-fed calls"): `acme-sync`, armed as "REC-001 next call", its folder listed twice while it recorded into `$D/rec-001.during`, then hung up and transcribed by the app. In S-17: the scenario's prelude (recording/TC-001 steps 1–4), hung up in window/TC-014; this case reads it and records nothing. On its own: `recording/TC-001` steps 1–6 first
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`, once per run: `app_profile.md` → "Recorded calls"): for `acme-sync` it kept what `echoctl capture --seconds 39` printed (`capture.out`, `capture.err`, `capture.exit`), the `meta.json` it closed, and what `echoctl transcribe` printed (`transcribe.out`) — the other way in to the same two tracks (step 4)
- Every command after the prelude of `recording/TC-001`; `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-001-next-call)"`, `P="$(python3 tests/fixtures/recordings.py path)/acme-sync"`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `cat "$D/rec-001.during"` | Both listings (2 s apart, while the call recorded) hold `meta.json`, `mic.wav` and `remote.wav`; each `.wav` is larger in the second listing (both tracks were being written) |
| 2 | Read `"$CALL/meta.json"` | `title` "REC-001 next call", `app` "teams", `platform` "macos", `ended_at` set, `stop_reason` "call_ended"; `tracks` lists `mic.wav` and `remote.wav`, each with `frames` above 0, their `frames` ÷ `sample_rate` within 0.5 s of each other, no event of kind `write_failed`; `audio` "deleted after transcription" |
| 3 | `ls "$CALL"; head -5 "$CALL/transcript.md"; python3 -c "import json,sys;s=json.load(open(sys.argv[1]+'/segments.json'));print(sorted({x['speaker'] for x in s}), len(s))" "$CALL"` | `transcript.md`, `segments.json` and `live.jsonl` (the live transcriber's lines, written while recording); the header names the model (`large-v3-turbo-q5_0`); both tracks were transcribed: speakers `['me', 'remote']` (or a speaker label for the remote track), six or more segments |
| 4 | The same through `echoctl capture`: `cat "$P/capture.exit"; tail -n 1 "$P/capture.out"; grep -E "(mic\|remote)\.wav" "$P/capture.err"; grep -o '"stop_reason": *"[^"]*"' "$P/meta.json"; grep -E "model:\|done in" "$P/transcribe.out"` | The exit code is 0 (it aborted at exit with 134 / -6 until #141); the last line of `capture.out` is the call folder `…/calls/<timestamp>-acme-sync`; `capture.err` has a line for `mic.wav` and one for `remote.wav`, each `— 3x.xs`-style seconds between 36 and 44, `0 restart(s)`; `stop_reason` "duration" (closed after the seconds asked); `transcribe.out` names the model and ends "done in …" |

## Expected Final State
The call is a folder whose mic and remote tracks were written as it ran, aligned, closed when it ended (at the hang-up for the watcher, after the requested seconds for `capture`), and transcribed from both tracks. Its words are scored in `transcription/TC-001`.

## Teardown
- Nothing to undo: it reads the run's own data and the prepared recordings

## Notes
- Since 2026-11-08 evening the case reads the scenario's main call (the watcher's recording) instead of a capture of its own, plus `prepare`'s capture of the same conversation for the `capture` command. Its "during" check is the listing recording/TC-001 step 4 takes while the call records (before: `prepare`'s `during.ls` of the capture).
- Both tracks come from files in a debug build (`ECHOCTL_TEST_ME_WAV`, `ECHOCTL_TEST_REMOTE_WAV`): the real tap of another app and the real mic device are the live check's (`recording/TC-023`, person).
- Run 2026-11-08 evening (S-17, file-fed): both wavs grew 8 KB → 72 KB in 2 s; tracks 39.83 s each (0.008 s apart), `call_ended`, 8 segments `me` + `remote`; `capture` exit -6 (#141), both lines 39.0 s, 0 restarts, `duration`.
- Run on 2026-11-07 (S-17, TestCall, its own capture then): 0% word errors, every speaker right; failed on the exit code only (#141).
