---
id: TC-024
title: A recording that starts before the call app holds a mic records silence on the mic track until it does
priority: high
type: functional
requirements: [F-REC-18]
module: recording
run: person
platform: macos
tags: [recording, devices, privacy, test-call, live-check]
---

# TC-024: A recording that starts before the call app holds a mic records silence on the mic track until it does

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- **The live check** (app_profile.md → "The live check"): real devices, so a person runs it. File-fed calls can't show it: their mic source is a file
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake --now <today>T12:00` (with `--now` it builds at any hour); **check `. <dir>/env` set `ECHOCTL_HOME` under `<dir>` before any `echoctl` step** (without it `capture` records into the real data folder)
- TestCall built, **BlackHole 2ch** (TestCall's microphone); Microphone and System Audio Recording granted to the terminal; no other TestCall call (port 7792)
- Prelude for every Bash call: `. <dir>/env; unset ECHOCTL_TEST_ME_WAV ECHOCTL_TEST_REMOTE_WAV ECHOCTL_TEST_CALL_FILE; D="$(dirname "$ECHOCTL_HOME")"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"`; `python3 tests/fixtures/audio/make_audio.py build very-short "$D/conv"` once

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl capture --seconds 25 --title "REC-024 early" > "$D/a.out" 2> "$D/a.err" & sleep 4; "$APP" serve --port 7792 --play "$D/conv/remote.wav" --quiet & sleep 0.5; "$APP" dial 127.0.0.1:7792 --me "$D/conv/mic.wav" --me-device "BlackHole 2ch" --quiet; wait` | The capture ends; `a.err` has `mic: capturing — … (system default, silent until the call app holds a mic)`, then `following`/`switched mic to "BlackHole 2ch"`, then `released its mic` |
| 2 | Peak level per second of `mic.wav` (`python3` with `wave`) in the call's folder | 0 for every second before the dial (0–4 s) and after the hang-up; the call's own line is there |
| 3 | `target/debug/echoctl capture --seconds 15 --title "REC-024 no call" 2> "$D/b.err"` (no TestCall call) | `b.err` has "… holds no mic Echo can open after 10 s: recording the default input …"; `mic.wav` is silent for its first 10 s |

## Expected Final State
The mic track holds the call app's mic only; the room before the call isn't recorded, and a call app that never reports a mic still gets its "Me" track after 10 s.

## Notes
- #201. Run 2026-11-08 on the dev Mac (steps 1–2): the dial at about 4.5 s, TestCall connected and started playing 1.5 s later; very-short's first "me" line starts about 5 s into its audio, so it is due at about 11 s. Peaks: 0 for 0–10 s (the room before the call: gated, not merely quiet — the Mac's mic was open on the default and zeroed), the line at 11–15 s, 0 after the hang-up; the log as above.
- After a fallback the track follows the default input, not an app's mic: a hang-up later doesn't release it, so the room after that call is recorded until the recording stops (the watcher's 10 s grace). Only in the fallback case.
- Step 3 records the room after 10 s, as decided (a virtual mic or a listen-only join must not leave "Me" empty for the whole call). Say so to anyone in the room.
