---
id: TC-001
title: Record the next call: armed, then recorded from connect to hang-up
priority: critical
type: functional
requirements: [F-REC-02, F-REC-08, F-REC-13]
module: recording
run: agent
platform: macos
tags: [recording, arm, second-instance]
external_id: TC-REC-001
---

# TC-001: Record the next call: armed, then recorded from connect to hang-up

**Module:** Recording | **Priority:** Critical | **Type:** Functional

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
| Conversation | acme-sync (36 s) |
| Arm title | REC-001 next call |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "REC-001 next call"`, then `target/debug/echoctl status` | arm prints "armed — recording starts when the call begins (expires in 15 min)"; `status` has `armed` with `title` "REC-001 next call" and `watcher.armed` true |
| 4 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 12; target/debug/echoctl status; wait` | At 12 s `status` shows `watcher.in_call` true, `watcher.recording` a folder path ending `-rec-001-next-call`, and `armed` null (the arm is used up); the call then plays out and ends by itself |
| 5 | Run the prelude, then `sleep 20; grep -o '"event":"[a-z_]*"' "$ECHOCTL_HOME/.echo/events.jsonl"` | In order: `call_started`, `recording_started`, `call_ended`, `recording_stopped` (about 12 s after the call ended: 2 s for the mic and 10 s of grace) |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-001-next-call)"; for i in $(seq 40); do [ -f "$CALL/transcript.md" ] && break; sleep 3; done; ls "$CALL"` | Within 2 minutes the folder has `transcript.md` and `segments.json`, and no `.wav` file (the app transcribed it and deleted the audio) |
| 7 | Read `meta.json` of that folder | `stop_reason` is "call_ended", `ended_at` is set, `title` is "REC-001 next call", `tracks` lists `mic.wav` and `remote.wav` |
| 8 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-001-next-call)"; grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"` | 2 or more: the last two turns ("Berlin first", "training room") are in the transcript; the first turn may be clipped by the seconds the watcher needs to notice the call, and is not asserted |

## Expected Final State
An armed Echo recorded the next call from its connect to its hang-up, closed it with reason "call_ended" and transcribed it, with no audio left behind.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Original case checked the tray icon turning armed and recording and the menu items; with `echoctl status` as the stand-in for the icon, that part is now `recording/TC-019` (person).
- Tools the steps use outside the usual rules (`app_profile.md`, Recording with the test call): `TestCall`, `nohup`, `grep` on the run's own files.
