---
id: TC-023
title: After the hang-up the mic track doesn't move to the Mac's microphone
priority: high
type: functional
requirements: [F-REC-17, F-REC-07]
module: recording
run: person
platform: macos
tags: [recording, devices, privacy, test-call, live-check]
---

# TC-023: After the hang-up the mic track doesn't move to the Mac's microphone

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- **The live check** (app_profile.md → "The live check"): real devices, so a person runs it, before a release. File-fed calls can't show it: their mic source is a file, not the device the call app holds or the Mac's own microphone
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has TestCall built (`tests/tools/testcall/build.sh`), **BlackHole 2ch** (TestCall's microphone: the user's lines are played into it), **BlackHole 16ch** (`brew install blackhole-16ch`: where TestCall plays the other side, so the call itself is silent in the room) and ffmpeg; Microphone and System Audio Recording are granted to the terminal running the steps and to TestCall; nothing else on this Mac uses BlackHole, and no other TestCall call runs (port 7792)
- The Mac's own microphone is the default input and its speakers the default output (no headphones): what `afplay` plays in the room reaches that microphone. Say so to anyone in the room first
- Every Bash call starts with the prelude, **the env without the file sources** (a fresh shell each time): `. <dir>/env; unset ECHOCTL_TEST_ME_WAV ECHOCTL_TEST_REMOTE_WAV ECHOCTL_TEST_CALL_FILE; D="$(dirname "$ECHOCTL_HOME")"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; W=tests/fixtures/wait.py; export D APP` (`ECHOCTL_CALL_APP_BUNDLE` stays: TestCall is the call app)
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- **Needs a second app instance** on the run's data and config (`app.lock` in the run's config folder, `app.rs`): if "Echo is already running" appears in `$D/app.out`, the case is BLOCKED

## Test Data

| Field | Value |
|-------|-------|
| Call | acme-sync (36 s) through TestCall, both ends on this Mac; TestCall hangs up about 37 s after the dial |
| The room after the hang-up | two-remotes' remote track played with `afplay` right after the hang-up: "Hi everyone, Omar is joining us…" (1–5 s), "…three questions about data retention…" (6–11 s), "…the audio be deleted right after transcription?" (18–24 s), "…the pilot plan…" (30–33 s) |
| Titles | REC-023 after hang-up (the watcher), REC-023 capture (`capture --seconds 50`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$D/conv/acme-sync"; python3 tests/fixtures/audio/make_audio.py build two-remotes "$D/room"` | Both print their folder, length and turns |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app` | "waited … for app"; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "REC-023 after hang-up"; $W 10 armed; CONV="$D/conv/acme-sync"; "$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" --speaker-device "BlackHole 16ch"; afplay "$D/room/remote.wav" & $W 40 event recording_stopped; wait` (`dial` returns at TestCall's hang-up; the room speaks from then on, through the watcher's 2 s debounce and 10 s grace) | The call records while it is on and is silent in the room; the room's lines play out loud after the hang-up; `recording_stopped` comes about 12 s after the hang-up |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-023-after-hang-up)"; $W 120 file "$CALL/transcript.md"; grep -c -i -e "Omar" -e "retention" -e "security team" "$CALL/transcript.md"; python3 -c "import json,sys;m=json.load(open(sys.argv[1]));[print(e) for e in m.get('events',[])]" "$CALL/meta.json"` | The count is 0: nothing said in the room after the hang-up is in the transcript; the mic track's events name BlackHole 2ch only (no `reopened` onto the Mac's microphone after the hang-up) |
| 5 | Run the prelude, then `CONV="$D/conv/acme-sync"; "$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" --speaker-device "BlackHole 16ch" & DIAL=$!; sleep 1; target/debug/echoctl capture --seconds 50 --title "REC-023 capture" > "$D/c023.out" 2> "$D/c023.err" & CAP=$!; wait $DIAL; afplay "$D/room/remote.wav"; wait $CAP; grep -E "capturing|reopened|restarting|(mic|remote)\.wav" "$D/c023.err"` (a capture longer than the call: it goes on 12 s after the hang-up while the room speaks) | The capture ends after 50 s; its log names BlackHole 2ch for the mic and never another input device after the hang-up |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-023-capture)"; target/debug/echoctl transcribe "$CALL"; grep -c -i -e "Omar" -e "retention" -e "security team" "$CALL/transcript.md"` | "done in …"; the count is 0: the room after the hang-up is not in the capture either |

## Expected Final State
Once the call app hangs up, nothing more reaches the mic track: the room is not recorded during the watcher's grace or the rest of a capture that outlasts the call.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- Was #140: after the hang-up the mic track reopened on the Mac's own microphone. Now, once the call app releases its mic (2 s grace), the track records silence and the log says "<app> released its mic: the mic track records silence until it takes one again". Checked 2026-11-08 with TestCall (`--quiet`, no BlackHole 16ch): no reopen after the hang-up.
- A person runs it: real devices, sound in the room, and the microphone's permission. File-fed calls (the agent runs) can't show it: `privacy/TC-002` is the other live-check case.
