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
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; W=tests/fixtures/wait.py; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
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
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "REC-001 next call"`, `$W 10 armed` (the watcher takes the arm at its next tick), then `target/debug/echoctl status` | arm prints "armed — recording starts when the call begins (expires in 15 min)"; `status` has `armed` with `title` "REC-001 next call" and `watcher.armed` true |
| 4 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & $W 20 recording rec-001-next-call; target/debug/echoctl status; wait` | Within 20 s of the dial `status` shows `watcher.in_call` true, `watcher.recording` a folder path ending `-rec-001-next-call`, and `armed` null (the arm is used up); the call then plays out and ends by itself |
| 5 | Run the prelude, then `$W 40 event recording_stopped; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -o '"event":"[a-z_]*"'` | In order: `call_started`, `recording_started`, `call_ended`, `recording_stopped` (about 12 s after the call ended: 2 s for the mic and 10 s of grace) |
| 6 | Run the prelude, then `$W 120 file "$ECHOCTL_HOME/calls/*-rec-001-next-call/transcript.md"; ls "$ECHOCTL_HOME"/calls/*-rec-001-next-call` | Within 2 minutes the folder has `transcript.md` and `segments.json`, and no `.wav` file (the app transcribed it and deleted the audio) |
| 7 | Read `meta.json` of that folder | `stop_reason` is "call_ended", `ended_at` is set, `title` is "REC-001 next call", `tracks` lists `mic.wav` and `remote.wav` |
| 8 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-001-next-call)"; grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"` | 2 or more: the last two turns ("Berlin first", "training room") are in the transcript; the first turn may be clipped by the seconds the watcher needs to notice the call, and is not asserted |

## Expected Final State
An armed Echo recorded the next call from its connect to its hang-up, closed it with reason "call_ended" and transcribed it, with no audio left behind.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Run on 2026-11-07 (S-17): PASS (with the `sleep 2` of step 3). Since 2026-11-08 the steps wait for the state (`wait.py`), not for fixed seconds. The transcript had one more Me line at 0:39, after the hang-up (#140): said in the room, picked up when the mic track reopened on the Mac's microphone for the 10 s grace (`recording/TC-014`).
- Original case checked the tray icon turning armed and recording and the menu items; with `echoctl status` as the stand-in for the icon, that part is now `recording/TC-019` (person).
- Tools the steps use outside the usual rules (`app_profile.md`, Recording with the test call): `TestCall`, `nohup`, `grep` on the run's own files.
