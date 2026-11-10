---
id: TC-007
title: Calls missed while Echo was quit get their minutes on the next start, written from the call alone and sent nowhere
priority: medium
type: functional
requirements: [F-MIN-18, F-MIN-14]
module: claude
run: agent
platform: macos
tags: [minutes, auto, second-instance, recordings]
ai: virtual
external_id: TC-CLA-006
auto: [tests/auto/specs/S-19.spec.mjs]
---

# TC-007: Calls missed while Echo was quit get their minutes on the next start, written from the call alone and sent nowhere

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai virtual --clock fast` (`ai: virtual`; the test clock: automatic minutes 90 s after a call in this debug build, `claude/TC-003`): the small set, hermetic, with the AI profile "Virtual model" (`tests/fixtures/model_server.py`, answering from its default script: `echoctl context`, the transcript read, `notes.md` and the action list written and applied) the default, so a minutes run is a real Claude Code run under Echo's guard with only the model scripted; Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; export D` (the env sets the file-fed call sources and the call app's id: nothing here records)
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"); `W=tests/fixtures/wait.py` in the prelude
- The dev app is started as `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"` (beside the user's own Echo, on the run's own config) after `config --unscheduled manual --scheduled manual` and `ai live off`; the window served on the data with clicks reaching it (`ECHOCTL_SERVE_IPC=1 … window --serve <port>`)
- `target/debug/echoctl config --auto-minutes true` set before the call (in S-19, `claude/TC-015` step 4); the dev app may be running (in S-19, `claude/TC-015`'s, kept for `cli/TC-005`): step 1 quits it

## Test Data

| Field | Value |
|-------|-------|
| Call | a copy of `minutes-call` (213 s: a decision and action items with owners and dates), title "CLA-007 missed" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl app quit` (if running); `target/debug/echoctl status` | `watcher` null: Echo is quit |
| 2 | Run the prelude, then `python3 tests/fixtures/recordings.py copy minutes-call --title "CLA-007 missed" > "$D/rec.out"; basename "$(cat "$D/rec.out")" | cut -c1-17 > "$D/cla-007.stamp"`; read the call's `auto` (`curl -s {{base_url}}/api/call/<folder>`) | `{"due": "at", "at": <the call's end + 90 s>}` |
| 3 | Leave Echo quit past that time: `sleep 105` after the copy (the delay and a tick: a check that nothing runs, not a wait for a condition); `ls "$(cat "$D/rec.out")"` | No `notes-run.json`: nothing ran while Echo was quit |
| 4 | Start the dev app (`$W 30 app`), then `$W 60 file "$ECHOCTL_HOME/calls/$(cat "$D/cla-007.stamp")*/notes-run.json"` and read it | Within a minute of the start: `"auto": true`, `started_at` within seconds of the start; `claude.log` starts "Echo starts Claude Code to write the minutes (automatically)" |
| 5 | Run the prelude, then `target/debug/echoctl pending --for "$ECHOCTL_HOME"` | `[]`: a session is never given the call (the app writes it) |
| 6 | `$W 300 sh "grep -qs '\"state\": *\"done\"' $ECHOCTL_HOME/calls/$(cat "$D/cla-007.stamp")*/notes-run.json"`; `ls` the call's folder (the run may rename it); read `notes.md`, `actions.json`, and the model's log (`grep -c -i 'SSO checklist' "$D/model/model-log.jsonl"`) | `notes.md` and `actions.json` are there: the virtual model's minutes ("The virtual model wrote these minutes…") and its one action item, "Me — send the follow-up notes", applied; the model's log holds the call's transcript (the run read it: the count is not 0, the checklist is said in the call). (Owners and dates a real model draws from the call: the release check of S-30.) |
| 7 | Read the call's `claude.log` (the window's "Show log") | Its first line: "Echo starts Claude Code to write the minutes (automatically)", with the model, the folder and "guard: every tool call is checked…"; the commands it ran are `echoctl context`, `type`, `actions` (and `title`) on this call, the reads and writes in its folder only; no `m365`, `compose`, `deliver` or other send; it ends "Minutes written. (Ns)" (each line after its time) |

## Expected Final State
A missed call is caught up by Echo at its next start, not by a session; the automatic run writes the minutes from the call alone and sends nothing.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`

## Notes
- Steps 6–7 were `claude/TC-003`'s (S-17) until 2026-11-08: S-17 now checks the automatic timing on the fake, and this is the scenario's automatic run (on the virtual model since 2026-11-10). Before, the case recorded a 213 s call and left Echo quit 16 minutes.
- "An hour later" became "past the minutes' time": the scheduler only asks whether the time has passed (`notes::auto_due`).
- Run on 2026-11-07 (S-19): PASS — call ended 13:20:54 with Echo quit, due 13:35:54; app started 13:36:33, the run started the same second (`auto: true`), done in 85 s; `pending --for` empty.
