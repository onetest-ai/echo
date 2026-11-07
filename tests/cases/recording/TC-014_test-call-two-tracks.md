---
id: TC-014
title: A test call is recorded as two tracks and transcribed word for word
priority: critical
type: functional
requirements: [F-REC-07, F-STT-01, F-STT-04, F-PRIV-03]
module: recording
run: agent
platform: macos
tags: [recording, test-call, tracks]
---

# TC-014: A test call is recorded as two tracks and transcribed word for word

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync: 6 turns, 36 s, Me + Dana (English) |
| Title | REC-014 acme-sync |
| Length | `capture --seconds 42` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 42 --title "REC-014 acme-sync" 2> "$D/capture.err"; wait` | Exit code 0; the last line printed is a folder `$ECHOCTL_HOME/calls/<timestamp>-rec-014-acme-sync`; `cat "$D/capture.err"` has a line for `mic.wav` and a line for `remote.wav`, each `— 4x.xs`-style seconds between 38 and 46 |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-014-acme-sync)"; ls "$CALL"` | The folder holds `meta.json`, `mic.wav`, `remote.wav` and `live.jsonl` (the live transcriber's lines) and no `transcript.md` |
| 4 | Read `"$CALL/meta.json"` | `title` is "REC-014 acme-sync", `app` "teams", `platform` "macos", `ended_at` is set, `stop_reason` is "duration"; `tracks` lists `mic.wav` and `remote.wav`, each with `frames` above 0 and no event of kind `write_failed` |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-014-acme-sync)"; cp "$CONV/expected.json" "$CALL/"; target/debug/echoctl transcribe "$CALL" --keep-audio` | "done in …"; the folder now has `transcript.md` and `segments.json`, and `mic.wav` and `remote.wav` are still there (`--keep-audio`) |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-014-acme-sync)"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true`; `wer` at most 0.15; all six turns have `speaker_ok` true and `lang_ok` true; `transcribed_in_silence` is an empty list |
| 7 | Read `"$CALL/transcript.md"` | The six turns are in time order; the user's turns are Me, Dana's are Remote; "Berlin first" and "SSO checklist" are spelled as said |

## Expected Final State
The call TestCall carried is a folder with a mic track and a remote track, a `meta.json` that says it was closed after the requested duration, and a transcript that scores 15% word errors or less with every speaker right.

## Teardown
- Nothing to undo: the call lives in the run's own data folder, which the lead removes

## Notes
- The mic track comes from "BlackHole 2ch" (the microphone TestCall holds as the call app); the remote track from the tap on TestCall's own audio, so only the call app is in it.
- Sequence proven on 2026-11-06: 0% word errors, every speaker right.
- Run on 2026-11-07 (S-17): 0% word errors, every speaker right; step 2 failed on the exit code only — `capture` saves the call, then aborts at exit with 134 (#141; a ggml-metal assertion, `GGML_ASSERT([rsets->data count] == 0)`), on every `capture` run that day.
- When TestCall hangs up, the mic track reopens on the system's default input for the seconds left (`mic: restarting — Teams released its mic; using the default input`, #140): on a Mac with a real microphone that records the room, so keep `--seconds` close to the call's length and nothing said in the room after it.
