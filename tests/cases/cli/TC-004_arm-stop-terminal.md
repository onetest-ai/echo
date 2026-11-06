---
id: TC-004
title: Arming and stopping from the terminal drives the recording like the menu
priority: medium
type: functional
requirements: [F-CLI-11, F-REC-02, F-REC-13]
module: cli
run: agent
platform: macos
tags: [cli, recording, second-instance]
external_id: TC-CLI-002
---

# TC-004: Arming and stopping from the terminal drives the recording like the menu

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), stopped 20 s after the dial |
| Arm title | CLI-004 terminal |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "CLI-004 terminal"`, then `target/debug/echoctl status` | arm prints "armed — recording starts when the call begins (expires in 15 min)"; `status` has `watcher.armed` true and `armed.title` "CLI-004 terminal" |
| 4 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 10; target/debug/echoctl status; sleep 10; target/debug/echoctl stop; sleep 4; target/debug/echoctl status; wait` | At 10 s `watcher.recording` is a folder path ending `-cli-004-terminal` and `watcher.armed` false. stop prints "stopping" and that folder. At 24 s `watcher.recording` is null and `armed` is null |
| 5 | Run the prelude, then `sleep 25; ls "$ECHOCTL_HOME/calls"; grep -o '"event":"[a-z_]*"' "$ECHOCTL_HOME/.echo/events.jsonl"` | One folder ends in `cli-004-terminal`; the events include `recording_started` then `recording_stopped` |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-cli-004-terminal)"; for i in $(seq 40); do [ -f "$CALL/transcript.md" ] && break; sleep 3; done; ls "$CALL"; grep -c "checklist" "$CALL/transcript.md"` | Within 2 minutes the folder has `transcript.md`; the count is 1 or more |
| 7 | Run the prelude, then `target/debug/echoctl stop` | "nothing to stop" (nothing is armed or recording) |

## Expected Final State
The terminal armed, and stopped, a recording exactly as the menu does; the call is in the data folder with its transcript.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Original steps read the menu-bar icon; `echoctl status` shows the same states (`watcher.armed`, `watcher.recording`). The icon itself is `recording/TC-019` (person).
