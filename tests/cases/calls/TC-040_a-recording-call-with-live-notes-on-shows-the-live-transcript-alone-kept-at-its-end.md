---
id: TC-040
title: A call recording with live notes on has the live transcript alone in its Transcript tab at full width, kept at its end unless the user scrolled up; the live notes box is in My notes
priority: high
type: functional
requirements: [F-LIVE-01]
module: calls
run: agent
platform: macos
tags: [calls, live-notes, transcript, recording, second-instance, file-fed]
---

# TC-040: A recording call with live notes on shows the live transcript alone, kept at its end; the live notes box is in My notes

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the live notes and the app's other Claude Code jobs are refused by the fake `claude`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"); every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` (the live transcript is the app's transcription during the call)
- **Needs a second app instance**, as in `recording/TC-003`; if `$D/app.out` says "already running" the case is BLOCKED, not failed
- The window is served on the same data at `{{base_url}}` with clicks reaching the app (`tests/fixtures/serve.sh up`); the page is loaded from `{{base_url}}/` first. Its viewport is set to 1201 x 500 (`browser_resize`) so that a few turns fill the live transcript (it is at most the window's height minus 240 px)
- No call is on and nothing is armed or recording; My notes is closed (`[data-panel-flip="notes"]` has `aria-pressed` false)
- **The window served here and the app are two processes.** The served window reads the app's files, not the live worker's memory: the live state it shows is always the empty one ("Waiting for enough to summarise"). What the model writes in the call (Now, decisions, action items, questions, the first update after about 90 s) is not checked here

## Test Data

| Field | Value |
|-------|-------|
| Conversation | minutes-call (213 s), held |
| Arm title | LIVE-040 live transcript (the folder ends `-live-040-live-transcript`) |
| Settings | languages en, `ai live on`, unscheduled and scheduled calls manual, automatic minutes off |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live on`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; `ai live on` prints "live notes during calls: on"; "waited … for app"; `status` prints `watcher` with a `pid`, `recording` null |
| 2 | Run the prelude, then `target/debug/echoctl arm --title "LIVE-040 live transcript"; $W 10 armed; $C start minutes-call --hold; $W 20 recording live-040` | "waited … for recording live-040" |
| 3 | Navigate to `{{base_url}}/#calls`, wait for the row `[data-call$="-live-040-live-transcript"]` (up to 15 s), click it, then click the tab "Transcript" | The row is marked Recording; the Transcript tab is selected |
| 4 | Wait until the pane holds at least 6 turns: `browser_evaluate` `() => document.querySelectorAll('#live-transcript .turn').length`, every 5 s for up to 2 minutes | A count of 6 or more |
| 5 | Snapshot the pane and `browser_evaluate` `() => { const lt = document.querySelector('#live-transcript'), pane = document.querySelector('#pane'); return { label: lt.getAttribute('aria-label'), find: !!document.querySelector('#find'), liveBox: !!pane.querySelector('[aria-label="Live notes"]'), ask: /Ask Claude about this call/.test(pane.textContent), wide: lt.getBoundingClientRect().width >= pane.clientWidth - 48, overflow: lt.scrollHeight > lt.clientHeight } }` | `{ label: "Transcript", find: false, liveBox: false, ask: false, wide: true, overflow: true }`: the live transcript (a region "Transcript") alone, no "Find in transcript" box, no live notes region and no "Ask Claude about this call" in it, as wide as the pane, and longer than it is tall |
| 6 | `browser_evaluate` `() => { const lt = document.querySelector('#live-transcript'); return lt.scrollTop + lt.clientHeight >= lt.scrollHeight - 40 }` | `true`: it is at its end (its last turn is the one in view) |
| 7 | Count the turns (step 4's expression), wait until the count is higher (every 3 s, up to 40 s), then run step 6's expression | The count grew and the expression is still `true`: the transcript followed its end |
| 8 | `browser_evaluate` `() => { const lt = document.querySelector('#live-transcript'); lt.scrollTop = 0; return lt.scrollTop }`; note the turn count, wait until it is higher (every 3 s, up to 40 s), then `browser_evaluate` `() => document.querySelector('#live-transcript').scrollTop` | The first returns 0; after the new turn the scroll position is still 0 (not pulled back to the end while the user reads up) |
| 9 | `browser_evaluate` `() => { const lt = document.querySelector('#live-transcript'); lt.scrollTop = lt.scrollHeight; return true }`, note the turn count, wait until it is higher (every 3 s, up to 40 s), then step 6's expression | `true` again: scrolled back to the end, it keeps following |
| 10 | Click My notes (`[data-panel-flip="notes"]`) | The right panel `#r-right` opens on My notes with the region `section[aria-label="Live notes"]`: the caption "Now", the paragraph "The first live notes appear after about 90 seconds of conversation.", a status line with the call's model name and "Waiting for enough to summarise", and the button "Live notes menu" (`[data-live-menu]`); the Transcript tab still shows the live transcript and still has no live notes region |

## Expected Final State
The Transcript tab of the recording call is the live transcript alone (no Find box, no live notes region), at the pane's full width, following its end while the user is at it and not moving while the user reads up; the live notes box is in My notes.

## Teardown
- Run the prelude, then `$C end; $W 60 not-recording; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Close the panel (`[aria-label="Close the panel"]`) and reset the viewport to 1201 x 868
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Only the layout half of F-LIVE-01 is run here. What the model writes (Now, Decisions, Action items, Open questions in My notes, each with a time jump, the first after about 90 s) needs the app's own window or a model that answers live notes, which the virtual model's scripts don't: person-run, on a real Claude Code or a model server. The notes' timeline once written is `window/TC-023`; the box's states are F-LIVE-02.
- With live notes off the same call has the ordinary Transcript tab and no live box: `calls/TC-041`.
- Written 2026-11-11 from the code (`14-call.js` `tabBody`, `18-live.js` `liveBox` and `liveTranscript`, `90-render.js` `ltStick`); not yet run. The rule that keeps the end is `ltStick` (within 40 px of the end): step 8 moves well past it.
