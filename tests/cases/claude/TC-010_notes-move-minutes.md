---
id: TC-010
title: A note added after the call moves the minutes' time
priority: medium
type: functional
requirements: [F-MIN-14, F-NOTES-06]
module: claude
run: agent
platform: macos
tags: [notes, minutes, auto, second-instance, test-call]
ai: local
external_id: TC-CLA-015
---

# TC-010: A note added after the call moves the minutes' time

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (a local model server: Ollama here, or the faster one of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused (`tests/fixtures/README.md`, "Real Claude Code runs on a local model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- The window is served on the same test data at `{{base_url}}` with clicks reaching the app (`ECHOCTL_SERVE_IPC=1`, `tests/fixtures/serve.sh up` or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Conversation | minutes-call (213 s) |
| Arm title | MIN-B notes move |
| Note after the call | Ask Kim for the pilot budget |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude | — |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes true`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | `auto_minutes` true (switched on now: calls that ended before stay without automatic minutes); `status` prints `watcher` with a `pid`; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build minutes-call "$D/mc"`, `target/debug/echoctl arm --title "MIN-B notes move"`, `sleep 2`, `CONV="$D/mc" nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait' > /dev/null 2>&1 &`, `sleep 10`, `target/debug/echoctl status` | minutes-call is 213 s (36 turns); `watcher.recording` is a folder ending `-min-b-notes-move` |
| 4 | Run the prelude, then `until [ -f "$(ls -d "$ECHOCTL_HOME"/calls/*-min-b-notes-move)/transcript.md" ]; do sleep 5; done`; open the call in the window (`[role=option][data-call$="min-b-notes-move"]`), Minutes tab | "Minutes in 15 min (HH:MM)", HH:MM = the call's end + 15 minutes |
| 5 | Wait until at least a minute after the call's end (a note within 30 s of the end doesn't count as "after the call"), then open My notes, type `Ask Kim for the pilot budget`, press Enter and click outside the box | The note is listed, "Saved"; the call's `my-notes.md` has it |
| 6 | Read the Minutes tab | The time moved to the note's time + 15 minutes, and the line "Moved: you edited your notes at HH:MM" names the note's time |
| 7 | Wait until `notes-run.json` appears in the call's folder (`until [ -f "$(ls -d "$ECHOCTL_HOME"/calls/2026*-min-b-notes-move)/notes-run.json" ]; do sleep 5; done`; the run may rename the folder when it ends) | `"auto": true` and `started_at` no earlier than the moved time, within 30 s of it |
| 8 | When it is "done", read `notes.md` | The note is in the minutes (e.g. an action item "Me — ask Kim for the pilot budget (noted after the call)") |

## Expected Final State
Editing notes after the call postponed the automatic minutes, and they include the note.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`

## Notes
- The case used to wait ten minutes before the note; any note more than 30 s after the call's end moves the time (`notes::auto_state`, "moved by notes"). A note within 30 s also sets the time to note + 15 minutes, but the tab doesn't call it moved.
- Run on 2026-11-07 (S-17): PASS — call ended 12:39:26, note at 12:41:34, the tab moved from 12:54 to 12:56 PM with "Moved: you edited your notes at 12:41 PM"; the run started 12:56:43 and kept the note.
