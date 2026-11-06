---
id: TC-014
title: Notes during a recording: Time inserts the call's time; terminal notes join them
priority: high
type: functional
requirements: [F-NOTES-10]
module: window
run: agent
platform: macos
tags: [notes, recording, second-instance]
external_id: TC-WIN-011
---

# TC-014: Notes during a recording: Time inserts the call's time; terminal notes join them

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- The window is served on the same test data at `{{base_url}}` (`tests/fixtures/serve.sh up`, or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), recorded on arm |
| Arm title | WIN-014 notes |
| Note typed | Ask Dana about the Berlin office date |
| Terminal note | Tray-style note from the terminal |
| Note after the call | Checklist goes out Friday |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "WIN-014 notes"` | arm prints "armed — recording starts when the call begins (expires in 15 min)" |
| 4 | Run the prelude, then `nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait' > /dev/null 2>&1 &`, then `sleep 10; target/debug/echoctl status` | `watcher.recording` is a folder ending `-win-014-notes` |
| 5 | Navigate to `{{base_url}}/#calls`, click the row `[data-call$="win-014-notes"]`, open My notes | The call opens; the notes panel has a "Time" button and a flag button; the call is shown as recording |
| 6 | Click "Time" | The notes text gets a time in square brackets like `[0:14] `, the call's own clock, not the wall clock |
| 7 | Type `half a thought` into the notes field without pressing Enter, then run the prelude and `target/debug/echoctl note Tray-style note from the terminal` | The command prints "noted in …/my-notes.md"; within 5 s the line `- [m:ss] Tray-style note from the terminal` (or `[mm:ss]`) shows in My notes, and `half a thought` is still in the field |
| 8 | Wait for the call to end and the recording to close (`sleep 30`), then add the note `Checklist goes out Friday` in My notes | The note is listed and the state reads "Saved" |
| 9 | Run the prelude, then `cat "$ECHOCTL_HOME"/calls/*-win-014-notes/my-notes.md` | The file holds the Time stamp line, the terminal note with its `[m:ss]` and `Checklist goes out Friday`, in that order |

## Expected Final State
Notes taken during and after a recording are in one `my-notes.md`, with the call's time where inserted.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- The tray's "Add a note…" is replaced by `echoctl note` (same file, same stamp: `calls::note`); the tray dialog itself is person-run.
- The "Time" button is shown only while the call's status is `recording`, so this case needs the app's watcher; with `window --serve` alone it is absent.
