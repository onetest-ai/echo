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
- The window is served on the same data at `{{base_url}}` with clicks reaching the app (`tests/fixtures/serve.sh up`); the page is loaded from `{{base_url}}/` first. The browser window is made short, 1201 x 500 (`browser_resize`), so that a few turns fill the live transcript and there is something to scroll
- No call is on and nothing is armed or recording; My notes is closed (`[data-panel-flip="notes"]` has `aria-pressed` false)
- **The window served here and the app are two processes.** The served window reads the app's files, not the live worker's memory: the live notes it shows are always the empty, waiting ones. What the model writes in the call (Now, decisions, action items, questions, the first update after about 90 s) is not checked here

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
| 5 | Snapshot the pane; then `browser_evaluate` `() => { const lt = document.querySelector('#live-transcript'), pane = document.querySelector('#pane'); return { region: lt.getAttribute('aria-label'), findBox: !!document.querySelector('#find'), liveNotes: !!pane.querySelector('[aria-label="Live notes"]'), askClaude: /Ask Claude about this call/.test(pane.textContent), widthShare: lt.getBoundingClientRect().width / pane.getBoundingClientRect().width, longerThanShown: lt.scrollHeight > lt.clientHeight } }` | `{ region: "Transcript", findBox: false, liveNotes: false, askClaude: false, widthShare: 0.9 or more, longerThanShown: true }`: the snapshot shows the live transcript alone in the tab (no "Find in transcript" box, no live notes, no "Ask Claude about this call"), spanning the pane's full width, with more turns than fit |
| 6 | `browser_evaluate` `() => { const lt = document.querySelector('#live-transcript'), t = lt.querySelectorAll('.turn'), r = t[t.length - 1].getBoundingClientRect(), v = lt.getBoundingClientRect(); return r.bottom <= v.bottom + 1 && r.top >= v.top }` | `true`: the last turn is fully in view (the transcript sits at its end) |
| 7 | Count the turns (step 4's expression), wait until the count is higher (every 3 s, up to 40 s), then run step 6's expression | The count grew and the expression is still `true`: the transcript followed its end, as the user reads along |
| 8 | Scroll up the way a user does: click on the transcript, press `Home` (`browser_press_key`; or the mouse wheel up, `browser_run_code_unsafe` `async (page) => { await page.mouse.wheel(0, -5000) }`). Then `browser_evaluate` `() => { const lt = document.querySelector('#live-transcript'), r = lt.querySelector('.turn').getBoundingClientRect(); return r.top >= lt.getBoundingClientRect().top }`; note the turn count, wait until it is higher (every 3 s, up to 40 s) and run the same expression again, then step 6's | The first expression is `true` (the first turn is in view); after the new turn it is still `true` and step 6's is `false`: the transcript did not jump back to the end while the user reads up |
| 9 | Scroll back down: click on the transcript, press `End` (or the mouse wheel down); `browser_evaluate` step 6's expression; note the turn count, wait until it is higher (every 3 s, up to 40 s), then step 6's expression again | `true` both times: back at the end, it follows the new turn again |
| 10 | Click My notes (`[data-panel-flip="notes"]`) | The right panel `#r-right` opens on My notes holding the live notes box (region "Live notes"): a "Now" part that says nothing has been summarised yet and that the first live notes come after about 90 seconds of conversation, a line with the call's model, and the box's menu (`[data-live-menu]`); the Transcript tab still shows the live transcript alone, with no live notes box in it |

## Expected Final State
The Transcript tab of the recording call is the live transcript alone (no Find box, no live notes region), at the pane's full width, following its end while the user is at it and not moving while the user reads up; the live notes box is in My notes.

## Teardown
- Run the prelude, then `$C end; $W 60 not-recording; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Close the panel (`[aria-label="Close the panel"]`) and reset the viewport to 1201 x 868
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Only the layout half of F-LIVE-01 is run here. What the model writes (Now, Decisions, Action items, Open questions in My notes, each with a time jump, the first after about 90 s) needs the app's own window or a model that answers live notes, which the virtual model's scripts don't: person-run, on a real Claude Code or a model server. The notes' timeline once written is `window/TC-023`; the box's states are F-LIVE-02.
- With live notes off the same call has the ordinary Transcript tab and no live box: `calls/TC-041`.
- Rewritten 2026-11-11 from F-LIVE-01 and the user's flow (the earlier draft had read its numbers from the code); not yet run. "Kept at its end" is checked by the last turn being in view, "not moving" by the first turn staying in view while a new one arrives.
