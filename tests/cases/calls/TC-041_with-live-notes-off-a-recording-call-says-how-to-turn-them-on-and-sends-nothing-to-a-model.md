---
id: TC-041
title: With live notes off, a question asked about a recording call is answered with how to turn them on, the call has the ordinary Transcript tab, and no live notes or usage are written
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
- A question put to a call (`echoctl live ask`) is a request file the app reads every second; the app refuses it when live notes are off and keeps the refusal as the answer (`livenotes::handle_requests`, `ask`). That is how a recording call says it, because with live notes off the window draws no live notes box until the call has an answer or notes

## Test Data

| Field | Value |
|-------|-------|
| Conversation | minutes-call (213 s), held: past the first live summary's 90 s a live-notes-on call would have written `live-notes.json` |
| Arm title | LIVE-041 notes off (the folder ends `-live-041-notes-off`) |
| Question | `What did we decide?` |
| Settings | languages en, `ai live off`, unscheduled and scheduled calls manual, automatic minutes off |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; `ai live off` prints "live notes during calls: off"; "waited … for app"; `status` prints `watcher` with a `pid`, `recording` null |
| 2 | Run the prelude, then `target/debug/echoctl arm --title "LIVE-041 notes off"; $W 10 armed; $C start minutes-call --hold; $W 20 recording live-041` | "waited … for recording live-041" |
| 3 | Navigate to `{{base_url}}/#calls`, wait for the row `[data-call$="-live-041-notes-off"]` (up to 15 s), click it, then click the tab "Transcript"; `browser_evaluate` `() => ({ live: !!document.querySelector('#live-transcript'), liveBox: !!document.querySelector('[aria-label="Live notes"]') })` | The row is marked Recording; `{ live: false, liveBox: false }`: no live transcript region and no live notes region (the ordinary Transcript tab: its text "Listening. The live transcript appears here as people speak." or, once turns are there, the "Find in transcript" box) |
| 4 | Run the prelude, then `L="$(ls -d "$ECHOCTL_HOME"/calls/*-live-041-notes-off)"; target/debug/echoctl live ask "$L" --kind ask "What did we decide?"; sleep 4; cat "$L/live-asks.jsonl"` | `live ask` prints "sent to Echo"; `live-asks.jsonl` holds one line with `"kind":"ask"`, `"question":"What did we decide?"`, `"answer":""` and `"error":"live notes are off — menu → AI for calls → Live notes during calls"` |
| 5 | Click My notes (`[data-panel-flip="notes"]`) and snapshot the panel `#r-right` | The region `section[aria-label="Live notes"]` is there with the red line (`.live-error`) "Live notes are off (menu → AI for calls → Live notes during calls)."; under the heading "Asked about this call" an answer headed "You asked" with "What did we decide?" and the error text "live notes are off — menu → AI for calls → Live notes during calls" (`.error`); no "✦" row |
| 6 | Run the prelude, then `sleep 95` (past the 90 s a live-notes-on call needs for its first summary; the call has gone on about 2 minutes), `L="$(ls -d "$ECHOCTL_HOME"/calls/*-live-041-notes-off)"; ls "$L"; target/debug/echoctl live show "$L"` | The folder holds neither `live-notes.json` nor `live-usage.json`; `live show` prints `"notes": null` and the `answers` of step 4 only: nothing was summarised and no model job was counted |
| 7 | Navigate to `{{base_url}}/#settings/ai` and read the row "Live notes during calls" | Its switch (`[data-set="live_notes"]`, `role=switch`) is off (`aria-checked` false): the setting step 5's text points to |

## Expected Final State
The recording call's My notes says live notes are off with where to turn them on (menu → AI for calls → Live notes during calls) and holds the refused question as its answer; the call's folder has no live notes and no usage file; the Transcript tab is the ordinary one.

## Teardown
- Run the prelude, then `$C end; $W 60 not-recording; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Close the panel (`[aria-label="Close the panel"]`)
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- "Nothing is sent to a model" is read from the files a summary would leave (`live-notes.json`, `live-usage.json`) after the 90 s of step 6, not from a model's log: the data's `claude` is the fake that refuses every run. A call recorded with live notes on, on this data, would keep a `live-state.json` of its own state; with them off the worker never starts (`livenotes::recording_started`: no state is set).
- Wording seen in `18-live.js` while writing this, not asserted: the box's paragraph (`.nl-now`) for a recording call with no notes says "The first live notes appear after about 90 seconds of conversation." even when live notes are off, under the red line that says they are off. Whether that line should be left out with live notes off is the owner's call; if a run shows it, report it rather than fail this case on it.
- With live notes off and no question asked, the window draws no box at all (`hasLive`): the pointer to the setting is the refused question's answer and the CLI's message ("live notes are off — menu → AI for calls → Live notes during calls"), and Settings → AI & minutes' row.
- Written 2026-11-11 from the code (`livenotes.rs` `ask`, `handle_requests`, `refused`; `18-live.js` `hasLive`, `liveBox`); not yet run.
