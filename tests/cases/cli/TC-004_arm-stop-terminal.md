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
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; W=tests/fixtures/wait.py; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
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
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "CLI-004 terminal"`, `$W 10 armed` (the watcher takes the arm at its next tick), then `target/debug/echoctl status` | arm prints "armed — recording starts when the call begins (expires in 15 min)"; `status` has `watcher.armed` true and `armed.title` "CLI-004 terminal" |
| 4 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & $W 20 recording cli-004-terminal; target/debug/echoctl status; sleep 10; target/debug/echoctl stop; $W 10 not-recording; target/debug/echoctl status; wait` | Once it records (`recording`, within 20 s), `watcher.recording` is a folder path ending `-cli-004-terminal` and `watcher.armed` false. stop prints "stopping" and that folder. 10 s later (the call's timeline) the stop; within seconds `watcher.recording` is null and `armed` is null |
| 5 | Run the prelude, then `$W 30 event call_ended; ls "$ECHOCTL_HOME/calls"; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -o '"event":"[a-z_]*"'` | One folder ends in `cli-004-terminal`; the events include `recording_started` then `recording_stopped` |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-cli-004-terminal)"; $W 120 file "$CALL/transcript.md"; ls "$CALL"` | Within 2 minutes the folder has `transcript.md` (what it holds is `recording/TC-015`'s check, not repeated here) |
| 7 | Run the prelude, then `target/debug/echoctl stop` | "nothing to stop" (nothing is armed or recording) |

## Expected Final State
The terminal armed, and stopped, a recording exactly as the menu does; the call is in the data folder with its transcript.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Run on 2026-11-07 (S-18): PASS; right after `arm`, `watcher.armed` was still false — it is true a tick later (hence the `sleep 2`).
- Original steps read the menu-bar icon; `echoctl status` shows the same states (`watcher.armed`, `watcher.recording`). The icon itself is `recording/TC-019` (person).
