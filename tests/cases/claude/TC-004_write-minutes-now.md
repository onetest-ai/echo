---
id: TC-004
title: Write minutes from the window, and for a call too short for automatic ones
priority: high
type: functional
requirements: [F-MIN-13, F-MIN-08]
module: claude
run: agent
platform: macos
tags: [minutes, recordings]
ai: virtual
external_id: TC-CLA-009
auto: [tests/auto/specs/S-19.spec.mjs]
---

# TC-004: Write minutes from the window, and for a call too short for automatic ones

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai virtual` (`ai: virtual`): the small set, hermetic, with the AI profile "Virtual model" (`tests/fixtures/model_server.py`, answering from its default script: `echoctl context`, the transcript read, `notes.md` and the action list written and applied) the default, so a minutes run is a real Claude Code run under Echo's guard with only the model scripted (`tests/fixtures/README.md`, "The virtual model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; export D` (the env sets the file-fed call sources and the call app's id: nothing here records)
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): the calls come from there, nothing is recorded or transcribed here
- The window served on the data with clicks reaching it: `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` after the prelude (no Echo app needed: the served window starts the run itself); the page loaded from `{{base_url}}/` first
- `target/debug/echoctl config --auto-minutes true --unscheduled manual --scheduled manual` (no Echo app running: nothing writes on its own)

## Test Data

| Field | Value |
|-------|-------|
| Long call | a copy of `minutes-call` (213 s), title "CLA-004 write now" |
| Short call | a copy of `very-short` (9 s), title "CLA-004 short" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/recordings.py copy minutes-call --title "CLA-004 write now"` | It prints the new call's folder: `minutes-call` (213 s) as captured and transcribed once by `prepare`, ending now |
| 2 | Run the prelude, then `python3 tests/fixtures/recordings.py copy very-short --title "CLA-004 short"` | It prints the short call's folder (9 s, transcribed) |
| 3 | `{{base_url}}/#calls`, click `[role=option][data-call$="cla-004-write-now"]`, Minutes tab | "Minutes in N min (HH:MM)" (15 on the real clock, 2 with `--clock fast`) and "Write now" (`[data-cmd="write-notes"]`); the status bar's "⏱ Minutes for “CLA-004 write now” in N min" |
| 4 | Click "Write now" | Within seconds "Writing minutes…" naming the profile and model ("Virtual model · virtual") and "Show log" |
| 5 | Wait until the call's `notes-run.json` says "done" (a few seconds on the virtual model; the run may retitle and rename the folder: find it by the `run_id`, or list `calls/`), then read its `claude.log` | The minutes are shown in the tab; `claude.log` starts "Echo starts Claude Code to write the minutes" with call, model, folder, plugin and "guard: every tool call is checked…", lists the commands Claude ran, and ends "Finished: ok · <time> · $<cost>" and "Minutes written. (<time>)" |
| 6 | Open `[role=option][data-call$="cla-004-short"]`, Minutes tab | "No minutes written automatically" and "The call is too thin for them (shorter than 3 minutes). Write them anyway if you want them.", and "Write minutes" |
| 7 | Click "Write minutes" and wait until its run is "done" | The minutes are written anyway (`notes.md` in its folder) |

## Expected Final State
Minutes can be written at once from the window, also for a call the gate skipped.

## Notes
- The "Minutes ready" notification is not checked (macOS shows it to the user).
- In S-19 its two runs are the scenario's runs A and B, and other stages hang off them: the user's notes go into the long call right after step 1 (`claude/TC-009`), a session is refused while step 4 writes and its claim on the short call is taken over by step 7 (`claude/TC-005`), the two runs are in folder A and in the data folder (`claude/TC-006`), and step 7's log names the model (`claude/TC-012`). With that claim, step 6's tab reads as written here (the claim comes after it) and step 7's "Write minutes" sits under "A Claude Code session is writing the minutes".
- With `--clock fast` the long call is due 90 s after step 1 (or after notes added to it): click step 4's Write now within them. Past that time, with no app running, the tab shows "Starting the minutes…" without the button.
- Run on 2026-11-07 (S-19, the network model server): PASS — the 4-minute call's minutes in 59 s ($0.32 as Claude Code counts it), retitled "SSO rollout — Berlin pilot, checklist, data retention"; the short call's in 44 s.
