---
id: TC-018
title: Two voices on the remote track stay Remote and are told apart
priority: medium
type: functional
requirements: [F-STT-04]
module: recording
run: agent
platform: macos
tags: [recording, test-call, speakers]
---

# TC-018: Two voices on the remote track stay Remote and are told apart

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps

## Test Data

| Field | Value |
|-------|-------|
| Conversation | two-remotes: Dana and Omar on the remote side (one track), the user on the mic (34 s) |
| Title | REC-018 two voices |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build two-remotes "$CONV"` | It prints the folder, the length and the turns of two-remotes; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 40 --title "REC-018 two voices" 2> "$D/capture.err"; wait` | Exit code 0; the last line printed is the call folder `…-rec-018-two-voices` |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-018-two-voices)"; cp "$CONV/expected.json" "$CALL/"; target/debug/echoctl transcribe "$CALL" --keep-audio` | "done in …"; `transcript.md` and `segments.json` exist |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-018-two-voices)"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true`; every remote turn has `speaker_ok` true (all of Dana's and Omar's lines are `remote`, none is Me) |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-018-two-voices)"; target/debug/echoctl speakers "$CALL"` | It lists two remote speakers (Speaker 1 and Speaker 2); the first lines of one are Dana's ("Hi everyone, Omar is joining us…"), of the other Omar's ("Hello. I have three questions…") |
| 6 | Read `"$CALL/transcript.md"` | The remote lines of the two voices carry two different speaker labels; the user's lines are Me |

## Expected Final State
One remote track, two voices: both are Remote, and the transcript tells them apart as two speakers.

## Teardown
- Nothing to undo: the call lives in the run's own data folder

## Notes
- If the detector merges the two voices into one speaker, step 5 fails: that is what this case checks (F-STT-04).
