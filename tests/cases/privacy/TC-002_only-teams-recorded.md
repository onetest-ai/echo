---
id: TC-002
title: Only the call app's audio is recorded as Remote
priority: critical
type: functional
requirements: [F-PRIV-02]
module: privacy
run: person
platform: macos
tags: [privacy, capture, test-call, live-check]
---

# TC-002: Only the call app's audio is recorded as Remote

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- **The live check** (app_profile.md → "The live check"): real devices, so a person runs it, before a release. File-fed calls can't show it: their remote source replaces the very tap this case is about
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has TestCall built (`tests/tools/testcall/build.sh`), **BlackHole 2ch** (TestCall's microphone: the user's lines are played into it), **BlackHole 16ch** (`brew install blackhole-16ch`: where TestCall plays the other side, so the call itself is silent in the room) and ffmpeg; Microphone and System Audio Recording are granted to the terminal running the steps and to TestCall; nothing else on this Mac uses BlackHole, and no other TestCall call runs (port 7792)
- Every Bash call starts with the prelude, **the env without the file sources** (a fresh shell each time): `. <dir>/env; unset ECHOCTL_TEST_ME_WAV ECHOCTL_TEST_REMOTE_WAV ECHOCTL_TEST_CALL_FILE; D="$(dirname "$ECHOCTL_HOME")"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D APP` (`ECHOCTL_CALL_APP_BUNDLE` stays: TestCall is the call app)
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- `afplay` (macOS) plays the second conversation out loud on the default output: it is not the call app and has no bundle id the app is told about. Say so to anyone in the room first

## Test Data

| Field | Value |
|-------|-------|
| Call | acme-sync (36 s) through TestCall, both ends on this Mac |
| Other audio | two-remotes' remote track, played with `afplay` from 8 s in |
| Title | REC-P002 other audio |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$D/conv/acme-sync"; python3 tests/fixtures/audio/make_audio.py build two-remotes "$D/other"` | Both print their folder, length and turns |
| 2 | Run the prelude, then `CONV="$D/conv/acme-sync"; "$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" --speaker-device "BlackHole 16ch" & sleep 2; target/debug/echoctl capture --seconds 34 --title "REC-P002 other audio" > "$D/p002.out" 2> "$D/p002.err" & CAP=$!; sleep 8; afplay "$D/other/remote.wav"; wait; tail -n 1 "$D/p002.out"` | During the call the other conversation plays on this Mac's speakers (the call itself is silent: it plays on BlackHole 16ch); the last line printed is the call folder `…-rec-p002-other-audio`; `p002.err` names the call app's tap and BlackHole 2ch as the sources |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-p002-other-audio)"; cp "$D/conv/acme-sync/expected.json" "$CALL/"; target/debug/echoctl transcribe "$CALL"; python3 tests/fixtures/audio/make_audio.py score "$CALL" | grep -A3 transcribed_in_silence` | "done in …"; `transcribed_in_silence` is `[]`: no line where the call had none, so nothing of the other audio came in on either track (the score's word errors are not this case's check: `transcription/TC-001`) |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-p002-other-audio)"; grep -c -i -e "Omar" -e "retention" -e "ninety days" -e "pilot plan" "$CALL/transcript.md"` | 0: nothing the other conversation said is in the transcript, as Remote or as Me |

## Expected Final State
Audio that is not the call app's never reaches the Remote track, and the call app's words are all there.

## Teardown
- Nothing to undo: the call lives in the run's own data folder; TestCall's two ends exit when the call is over

## Notes
- This case transcribes its own capture: what was tapped is the subject, and the words are the evidence.
- Run on 2026-11-07 (S-18, agent, TestCall without BlackHole 16ch): PASS — WER 0.0, 0 lines of the other conversation. After TestCall hung up (about 38 s) the mic reopened on the Mac's microphone until the capture ended (42 s): #140, `recording/TC-023`. The capture here ends 36 s after the dial, after the call's last line (35 s) and before the hang-up (37 s), so that defect can't leak the other conversation into this case.
- 2026-11-08: `run: person` (the live check): file-fed calls replace the tap. TestCall plays through its own process and the app's tap follows that process, so `afplay` is outside it. The user's mic is BlackHole 2ch, not an acoustic microphone; the faint pickup by a real microphone on speakers remains `recording/TC-008`'s.
