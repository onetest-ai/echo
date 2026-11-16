---
id: TC-041
title: With live notes off, opening a recording call says they are off and where to turn them on, with nothing asked first and no promise of notes, and nothing is sent to a model
priority: medium
type: functional
requirements: [F-LIVE-04]
module: calls
run: agent
platform: macos
tags: [calls, live-notes, recording, second-instance, file-fed]
---

# TC-041: With live notes off a recording call says how to turn them on, and nothing is sent to a model

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"); every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0`
- **Needs a second app instance**, as in `recording/TC-003`; if `$D/app.out` says "already running" the case is BLOCKED, not failed
- The window is served on the same data at `{{base_url}}` with clicks reaching the app (`tests/fixtures/serve.sh up`); the page is loaded from `{{base_url}}/` first; My notes is closed
- No call is on and nothing is armed or recording

## Test Data

| Field | Value |
|-------|-------|
| Conversation | minutes-call (213 s), held: past the first live summary's 90 s a live-notes-on call would have written its live notes |
| Arm title | LIVE-041 notes off (the folder ends `-live-041-notes-off`) |
| Settings | languages en, `ai live off`, unscheduled and scheduled calls manual, automatic minutes off |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; `ai live off` prints "live notes during calls: off"; "waited … for app"; `status` prints `watcher` with a `pid`, `recording` null |
| 2 | Run the prelude, then `target/debug/echoctl arm --title "LIVE-041 notes off"; $W 10 armed; $C start minutes-call --hold; $W 20 recording live-041-notes-off` | "waited … for recording live-041-notes-off" |
| 3 | Navigate to `{{base_url}}/#calls`, wait for the row `[data-call$="-live-041-notes-off"]` (up to 15 s), click it; ask nothing. Snapshot the call, then click My notes (`[data-panel-flip="notes"]`) and snapshot the panel `#r-right` | The row is marked Recording. Between the call's view and My notes, the screen says that live notes are off and where to turn them on (Settings → AI & minutes, "Live notes during calls"), with no question asked first |
| 4 | In the snapshots of step 3, look for the words "first live notes appear after about 90 seconds" | They appear nowhere: a call with live notes off promises no notes |
| 5 | Run the prelude, then `sleep 95` (past the 90 s a live-notes-on call needs for its first summary), `L="$(ls -d "$ECHOCTL_HOME"/calls/*-live-041-notes-off)"; ls "$L"` | The folder holds neither `live-notes.json` nor `live-usage.json`: nothing was summarised and no model job was counted (the data's `claude` refuses every run, so these files are the evidence); no file of a question either (`live-asks.jsonl` absent) |
| 6 | Navigate to `{{base_url}}/#settings/ai` and read the row "Live notes during calls" | Its switch (`[data-set="live_notes"]`, `role=switch`) is off (`aria-checked` false): the setting step 3's text points to |

## Expected Final State
Opening the recording call, with no question asked, shows that live notes are off and where to turn them on (Settings → AI & minutes), without promising notes in 90 seconds; the call's folder has no live notes and no usage file.

## Teardown
- Run the prelude, then `$C end; $W 60 not-recording; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Close the panel (`[aria-label="Close the panel"]`)
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Steps 3 and 4 assert F-LIVE-04 as written, the behaviour #399 fixed (before it, the window said nothing about live notes being off until someone asked a question, and its box promised notes in about 90 seconds). Do not "fix" the case by asking a question first.
- "Nothing is sent to a model" is read from the files a summary would leave after the 90 s of step 5, not from a model's log.
- Rewritten 2026-11-11 from F-LIVE-04 and #399; not yet run.
