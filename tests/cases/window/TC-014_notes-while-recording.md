---
id: TC-014
title: Notes during a recording carry the call's time; terminal notes join them
priority: high
type: functional
requirements: [F-NOTES-10]
module: window
run: agent
platform: macos
tags: [notes, recording, second-instance]
external_id: TC-WIN-011
---

# TC-014: Notes during a recording carry the call's time; terminal notes join them

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
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
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "WIN-014 notes"` | arm prints "armed — recording starts when the call begins (expires in 15 min)" |
| 4 | Run the prelude, then `nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait' > /dev/null 2>&1 &`, then `sleep 10; target/debug/echoctl status` | `watcher.recording` is a folder ending `-win-014-notes` |
| 5 | Navigate to `{{base_url}}/#calls`, click the row `[role=option][data-call$="win-014-notes"]` (the Record pill's title button carries the same `data-call`), open My notes (`[aria-label="My notes"]`) | The call opens on its live transcript; the row and the Transcript tab say Recording; the notes box `[aria-label="New note"]` has the placeholder "Note at m:ss — Enter to add" (the call's clock), a "⚑ Flag" button and the line "The time is taken when you start typing." |
| 6 | Type `Ask Dana about the Berlin office date` in the notes box and press Enter | The note is listed as `mm:ss Ask Dana about the Berlin office date` with the call's time (seconds into the call, e.g. `00:15`), not the wall clock |
| 7 | Type `half a thought` into the notes box without pressing Enter, then run the prelude and `target/debug/echoctl note Tray-style note from the terminal`; then click outside the box (e.g. the call's title) | The command prints "noted in …/my-notes.md". While the box has the focus the list is not redrawn (`typingNotes()` holds it); within 5 s of the click outside, `mm:ss Tray-style note from the terminal` shows in My notes and `half a thought` is still in the box |
| 8 | Wait for the call to end and the recording to close (`sleep 30`), then replace the box's text with `Checklist goes out Friday` and press Enter | The note is listed without a time (the call is over) and the panel reads "Saved" |
| 9 | Run the prelude, then `cat "$ECHOCTL_HOME"/calls/*-win-014-notes/my-notes.md` | Three lines in this order: `- [mm:ss] Ask Dana about the Berlin office date`, `- [mm:ss] Tray-style note from the terminal`, `- Checklist goes out Friday` |

## Expected Final State
Notes taken during and after a recording are in one `my-notes.md`, with the call's time where inserted.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- The tray's "Add a note…" is replaced by `echoctl note` (same file, same stamp: `calls::note`); the tray dialog itself is person-run.
- The note's time comes from the call's recording clock, so this case needs the app's watcher; with `window --serve` alone the call isn't recording.
- "Time" (insert the call's time) is in My notes' "Edit as text" view (`[aria-label="Edit as text"]`, then `[data-cmd="stamp"]`), shown only while recording; the default view times each note from its first key instead (steps 5–6).
- Run on 2026-11-07 (S-17): PASS on steps 1–9 as written here (the Edit-as-text "Time" not exercised).
