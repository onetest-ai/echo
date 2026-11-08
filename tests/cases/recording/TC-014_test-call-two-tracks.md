---
id: TC-014
title: A test call is recorded as two tracks, closed after its duration, and transcribed from them
priority: critical
type: functional
requirements: [F-REC-07, F-STT-01, F-STT-04, F-PRIV-03]
module: recording
run: agent
platform: macos
tags: [recording, test-call, tracks, recordings]
---

# TC-014: A test call is recorded as two tracks, closed after its duration, and transcribed from them

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`, once per run: `app_profile.md` → "Recorded calls"). For `acme-sync` it ran, on this Mac, the same commands this case used to run itself: TestCall `serve` + `dial` on 127.0.0.1 (the user's line into BlackHole 2ch), `echoctl capture --seconds 39 --title acme-sync` (stdout `capture.out`, stderr `capture.err`, exit code `capture.exit`), an `ls` of the folder 8 s in (`during.ls`), `echoctl transcribe` (`transcribe.out`) and an `ls` after it (`after.ls`). This case reads that evidence; it records and transcribes nothing
- Every command after `. <dir>/env` on test data built by `tests/fixtures/make-data.sh <dir>`, and `P="$(python3 tests/fixtures/recordings.py path)/acme-sync"`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `cat "$P/capture.exit"; tail -n 3 "$P/capture.out"; grep -E "(mic|remote)\.wav" "$P/capture.err"` | The exit code is 0, or 134 / -6 (`capture` aborts at exit after saving, #141); the last line of `capture.out` is the call folder `…/calls/<timestamp>-acme-sync`; `capture.err` has a line for `mic.wav` and one for `remote.wav`, each `— 3x.xs`-style seconds between 36 and 44, `0 restart(s)` on `remote.wav` |
| 2 | `cat "$P/during.ls"` | While the call recorded, the folder held `meta.json`, `mic.wav` and `remote.wav` |
| 3 | Read `"$P/meta.json"` (the folder's `meta.json` as `capture` closed it) | `title` "acme-sync", `app` "teams", `platform` "macos", `ended_at` set, `stop_reason` "duration"; `tracks` lists `mic.wav` and `remote.wav`, each with `frames` above 0, their `frames` ÷ `sample_rate` within 0.5 s of each other, no event of kind `write_failed` |
| 4 | `cat "$P/after.ls"; head -8 "$P/transcribe.out"` | After the transcription the folder has `transcript.md`, `segments.json` and `live.jsonl` (the live transcriber's lines, written while recording) and no `.wav`; `transcribe.out` names the model and ends "done in …" |
| 5 | `CALL="$(python3 tests/fixtures/recordings.py copy acme-sync --title "REC-014 acme-sync")"; python3 -c "import json,sys;s=json.load(open(sys.argv[1]+'/segments.json'));print(sorted({x['speaker'] for x in s}), len(s))" "$CALL"` | Both tracks were transcribed: speakers `['me', 'remote']` (or a speaker label for the remote track), six or more segments |

## Expected Final State
The call TestCall carried is a folder with a mic track and a remote track, aligned, a `meta.json` that says it was closed after the requested duration, and a transcript of both tracks. Its words are scored in `transcription/TC-001`.

## Teardown
- Nothing to undo: the copy lives in the run's own data folder

## Notes
- The mic track comes from "BlackHole 2ch" (the microphone TestCall holds as the call app); the remote track from the tap on TestCall's own audio, so only the call app is in it.
- `prepare` stops the capture 3.5 s after the conversation, before TestCall hangs up: after the hang-up the mic reopens on the Mac's microphone (#140) and a word said in the room would land in every copy. That fallback is seen in the watcher cases (`recording/TC-001`), not here.
- Run on 2026-11-07 (S-17, its own capture then): 0% word errors, every speaker right; step 2 failed on the exit code only (#141).
