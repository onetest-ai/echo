---
id: TC-002
title: Only the call app's audio is recorded as Remote
priority: critical
type: functional
requirements: [F-PRIV-02]
module: privacy
run: agent
platform: macos
tags: [privacy, capture, test-call]
---

# TC-002: Only the call app's audio is recorded as Remote

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; W=tests/fixtures/wait.py; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- `afplay` (macOS) plays the second conversation on the default output; it is not the call app and has no bundle id the app is told about

## Test Data

| Field | Value |
|-------|-------|
| Call | acme-sync (36 s) through TestCall |
| Other audio | two-remotes' remote track, played with `afplay` from 8 s in |
| Title | REC-P002 other audio |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build two-remotes "$D/other"` | It prints the folder, the length and the turns of two-remotes |
| 3 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 39 --title "REC-P002 other audio" 2> "$D/capture.err" & CAP=$!; sleep 8; afplay "$D/other/remote.wav"; wait` | Exit code 0; during the call the other conversation plays on this Mac's speakers; the last line printed is the call folder `…-rec-p002-other-audio` |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-p002-other-audio)"; cp "$CONV/expected.json" "$CALL/"; target/debug/echoctl transcribe "$CALL"; python3 tests/fixtures/audio/make_audio.py score "$CALL" | grep -A3 transcribed_in_silence` | "done in …"; `transcribed_in_silence` is `[]`: no line where the call had none, so nothing of the other audio came in on either track (the score's word errors are not this case's check: `transcription/TC-001`) |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-p002-other-audio)"; grep -c -i -e "Omar" -e "retention" -e "ninety days" -e "pilot plan" "$CALL/transcript.md"` | 0: nothing the other conversation said is in the transcript, as Remote or as Me |

## Expected Final State
Audio that is not the call app's never reaches the Remote track, and the call app's words are all there.

## Teardown
- Nothing to undo: the call lives in the run's own data folder

## Notes
- This case transcribes its own capture: what was tapped is the subject, and the words are the evidence.
- Run on 2026-11-07 (S-18): PASS — WER 0.0, 0 lines of the other conversation. `afplay` plays out loud in the room, and after TestCall hangs up (about 38 s) the mic reopens on the Mac's microphone until the capture ends (42 s): the other conversation's last line ends at about 38 s, so keep the timing as it is.
- TestCall plays through its own process; the app's audio tap follows it, so `afplay` is outside it. On headphones or speakers makes no difference here: the user's mic is BlackHole, not an acoustic microphone. The faint pickup by a real microphone on speakers remains a person's check.
