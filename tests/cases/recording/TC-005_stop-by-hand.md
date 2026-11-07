---
id: TC-005
title: Stopping by hand ends the recording, not the call
priority: critical
type: functional
requirements: [F-REC-05]
module: recording
run: agent
platform: macos
tags: [recording, stop, second-instance, test-call]
external_id: TC-REC-005
---

# TC-005: Stopping by hand ends the recording, not the call

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- The window is served on the same test data at `{{base_url}}` with clicks reaching the app (`ECHOCTL_SERVE_IPC=1`, `tests/fixtures/serve.sh up` or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s); stopped from the window about 15 s in |
| Arm title | REC-005 pill stop |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "REC-005 pill stop"`, `sleep 2`, then `nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait' > /dev/null 2>&1 &`, `sleep 6`, `target/debug/echoctl status` | `watcher.recording` is a folder ending `-rec-005-pill-stop` |
| 4 | Load `{{base_url}}/`; in the top bar's group "Recording" click the stop button `[data-cmd="stop-ask"]` | The pill turns into "Stop recording?" with the buttons "Stop" (`.pill.rec [data-cmd="stop"]`) and "Keep recording" (`[data-cmd="stop-keep"]`) |
| 5 | Click "Keep recording", wait 3 s | The pill shows the recording again (its clock still running); `echoctl status` still has `watcher.recording` |
| 6 | Click the stop button again, then "Stop"; within 4 s run the prelude and `target/debug/echoctl status` | `watcher.recording` is null, `watcher.in_call` is still true (the call goes on), `armed` is null |
| 7 | Run the prelude, then wait for the call to end (`until ! pgrep -f "testcall dial" > /dev/null; do sleep 2; done; sleep 20`) and `tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -o '"event":"[a-z_]*"'`; `ls "$ECHOCTL_HOME/calls" | grep -c rec-005` | In order `call_started`, `recording_started`, `recording_stopped` (reason "user", about 15 s), `transcribed`, `call_ended`: one recording only, nothing re-armed after the stop; one folder |
| 8 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-005-pill-stop)"; grep -o '"stop_reason": *"[a-z_]*"' "$CALL/meta.json"; grep -c -e "Madrid" -e "training room" "$CALL/transcript.md"` | `"stop_reason": "user"`; the count is 0: the transcript ends where it was stopped |

## Expected Final State
The window's Stop asked first, "Keep recording" kept it, "Stop" ended the recording while the call went on, and nothing more of that call was recorded.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- TestCall stands in for Teams: "the call goes on" is `watcher.in_call` staying true and `call_ended` coming after the stop. The tray's "Stop recording" is `recording/TC-019` (person); `echoctl stop` is `recording/TC-015`, Ctrl-C of a capture `recording/TC-016`.
- Run on 2026-11-07 (S-18): PASS — stop at 13:03:12 after a "Keep recording", call ended 13:03:36, one recording, `stop_reason` "user", no Madrid line.
