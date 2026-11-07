---
id: TC-009
title: Notes taken during a call shape its minutes
priority: high
type: functional
requirements: [F-NOTES-10, F-MIN-13]
module: claude
run: agent
platform: macos
tags: [notes, minutes, second-instance, test-call]
ai: local
external_id: TC-CLA-008
---

# TC-009: Notes taken during a call shape its minutes

**Module:** Claude Code | **Priority:** High | **Type:** Functional

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
| Conversation | minutes-call (213 s: long enough for automatic minutes) |
| Arm title | MIN-A catch-up |
| Terminal note (the tray's "Add a note…") | ask legal about the contract |
| Window note | Berlin needs ten seats in the training room |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude | — |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes true`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | `auto_minutes` true (switched on now: calls that ended before stay without automatic minutes); `status` prints `watcher` with a `pid`; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build minutes-call "$D/mc"`, `target/debug/echoctl arm --title "MIN-A catch-up"`, `sleep 2`, `CONV="$D/mc" nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait' > /dev/null 2>&1 &`, `sleep 10`, `target/debug/echoctl status` | minutes-call is 213 s (36 turns); `watcher.recording` is a folder ending `-min-a-catch-up` |
| 4 | Run the prelude, then `target/debug/echoctl note ask legal about the contract` | "noted in …/my-notes.md" |
| 5 | In the window: `{{base_url}}/#calls`, click the row `[role=option][data-call$="min-a-catch-up"]`, open My notes, type `Berlin needs ten seats in the training room` in `[aria-label="New note"]` and press Enter; click outside the box | My notes lists both, each with the call's time (`00:09 ask legal about the contract`, `00:14 Berlin needs ten seats…`), and reads "Saved" |
| 6 | Run the prelude, then `until [ -f "$(ls -d "$ECHOCTL_HOME"/calls/*-min-a-catch-up)/transcript.md" ]; do sleep 5; done`, then the minutes as in `claude/TC-003` (automatic, 15 minutes after the call), or — standalone — the Minutes tab's "Write now"; wait until `notes-run.json` says `"state": "done"` | The minutes are written (`notes.md`, `actions.json`) |
| 7 | Run the prelude, then `grep -n -i -e "legal" -e "ten seats" "$(ls -d "$ECHOCTL_HOME"/calls/*-min-a-catch-up)/notes.md"` | Both notes are in the minutes, in their structure (e.g. an action item "Me — Ask legal about the contract", "ten seats" with the training room), not pasted as a list of notes |

## Expected Final State
The user's own notes, from the terminal and from the window, are part of the minutes.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`

## Notes
- `echoctl note` writes what the tray's "Add a note…" writes (`calls::note`); the tray's dialog itself is person-run (`recording/TC-019`'s reason).
- The model's wording differs from run to run: the check is that each note's content is there.
- Run on 2026-11-07 (S-17, the network model server): PASS — "Me — Ask legal about the contract (user note, [00:09])" in Action items, "Berlin needs ten seats in it (user note)" in the Highlights; the run took 116 s.
