---
id: TC-004
title: Write minutes from the window, and for a call too short for automatic ones
priority: high
type: functional
requirements: [F-MIN-13, F-MIN-08]
module: claude
run: agent
platform: macos
tags: [minutes, test-call]
ai: local
external_id: TC-CLA-009
---

# TC-004: Write minutes from the window, and for a call too short for automatic ones

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (Ollama here, or the faster server of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused (`tests/fixtures/README.md`, "Real Claude Code runs on a local model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model linked into the test config (`ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, `target/debug/echoctl model use large-v3-turbo-q5_0`, `config --languages en`); TestCall, BlackHole 2ch and ffmpeg as in `recording/TC-014`
- The window served on the data with clicks reaching it: `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` after the prelude (no Echo app needed: the served window starts the run itself); the page loaded from `{{base_url}}/` first
- `target/debug/echoctl config --auto-minutes true --unscheduled manual --scheduled manual` (no Echo app running: nothing writes on its own)

## Test Data

| Field | Value |
|-------|-------|
| Long call | minutes-call (213 s), title "CLA-004 write now" |
| Short call | very-short (9 s), title "CLA-004 short" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build minutes-call "$D/minutes-call"; "$APP" serve --port 7792 --play "$D/minutes-call/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$D/minutes-call/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 220 --title "CLA-004 write now" > "$D/rec.out" 2> "$D/rec.err"; wait; target/debug/echoctl transcribe "$(cat "$D/rec.out")"` | "done in …" |
| 2 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$D/very-short"; "$APP" serve --port 7792 --play "$D/very-short/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$D/very-short/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 12 --title "CLA-004 short" > "$D/rec.out" 2> "$D/rec.err"; wait; target/debug/echoctl transcribe "$(cat "$D/rec.out")"` | "done in …" |
| 3 | `{{base_url}}/#calls`, click `[role=option][data-call$="cla-004-write-now"]`, Minutes tab | "Minutes in 15 min (HH:MM)" and "Write now" (`[data-cmd="write-notes"]`); the status bar's "⏱ Minutes for “CLA-004 write now” in 15 min" |
| 4 | Click "Write now" | Within seconds "Writing minutes…" naming the profile and model ("Qwen · … · <model>") and "Show log" |
| 5 | Wait until the call's `notes-run.json` says "done" (about a minute on the network server; the run may retitle and rename the folder: find it by the `run_id`, or list `calls/`), then read its `claude.log` | The minutes are shown in the tab; `claude.log` starts "Echo starts Claude Code to write the minutes" with call, model, folder, plugin and "guard: every tool call is checked…", lists the commands Claude ran, and ends "Finished: ok · <time> · $<cost>" and "Minutes written. (<time>)" |
| 6 | Open `[role=option][data-call$="cla-004-short"]`, Minutes tab | "No minutes written automatically" and "The call is too thin for them (shorter than 3 minutes). Write them anyway if you want them.", and "Write minutes" |
| 7 | Click "Write minutes" and wait until its run is "done" | The minutes are written anyway (`notes.md` in its folder) |

## Expected Final State
Minutes can be written at once from the window, also for a call the gate skipped.

## Notes
- The "Minutes ready" notification is not checked (macOS shows it to the user).
- Run on 2026-11-07 (S-19, the network model server): PASS — the 4-minute call's minutes in 59 s ($0.32 as Claude Code counts it), retitled "SSO rollout — Berlin pilot, checklist, data retention"; the short call's in 44 s.
