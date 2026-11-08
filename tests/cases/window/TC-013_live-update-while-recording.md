---
id: TC-013
title: The window updates on its own when a recording ends
priority: medium
type: functional
requirements: [F-CALLS-18]
module: window
run: agent
platform: macos
tags: [calls, recording, second-instance]
external_id: TC-WIN-007
---

# TC-013: The window updates on its own when a recording ends

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake`, its env feeding calls from files (`recording/TC-001`'s Preconditions); every Bash call starts with the prelude of `recording/TC-001`
- The window is served on the same test data at `{{base_url}}` (`tests/fixtures/serve.sh up`, or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first and **not reloaded during the case**
- **A call that the app records and transcribes by itself while the window watches.** In S-17: the main call "REC-001 next call" — the prelude loaded the page and opened Initech planning (step 1) before it started the call (step 2), window/TC-014 opened the recording from the list (the same check as step 3) and hung it up; `<slug>` is `rec-001-next-call`. On its own: step 2 here; `<slug>` is `win-013-live-update`

## Test Data

| Field | Value |
|-------|-------|
| Call that stays open | Initech planning (test data), on its Transcript tab |
| Recorded call | acme-sync (36 s), armed as "REC-001 next call" (S-17) or "WIN-013 live update" (on its own) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[role=option][data-call$="initech-planning"]`, open its Transcript tab | The two turns of Initech planning are shown; the tab Transcript is selected |
| 2 | On its own: `recording/TC-001` steps 1–2 (the speech model, the app); then run the prelude and `target/debug/echoctl arm --title "WIN-013 live update"; $W 10 armed; $C start acme-sync; $W 20 recording win-013-live-update` | "waited … for recording": the app records the call; it hangs up by itself at 37 s |
| 3 | Without reloading, wait up to 15 s (`browser_wait_for` the call's title) | A new row `[role=option][data-call$="<slug>"]` appears in the Calls list by itself, marked Recording |
| 4 | Click the new row (in S-17 it is open already: window/TC-014 left it on screen), then, without reloading, wait for the app to finish (`$W 90 event transcribed`) and look at its Transcript tab | Within 15 s of `transcribed` the open call no longer says Recording and its Transcript tab shows the final transcript: "Berlin first" and "training room" in the last turns |
| 5 | Click the Initech planning row and look at its tab | The row is still in the list; it opens on Transcript as before (the refreshes did not reset the tab you had chosen) |

## Expected Final State
A call recorded and transcribed behind the window's back shows up and fills in by itself; what was open stays as it was.

## Teardown
- On its own: run the prelude, then `target/debug/echoctl app quit`. In S-17: skipped (the app stays)

## Notes
- Since 2026-11-08 evening the case hangs off a watcher recording (in S-17 the main call): the app transcribes it itself, so "fills in" is the real path. Before, it captured a 9 s call with `echoctl capture` and gave it the prepared transcript (`recordings.py finish`); that copy kept its two tracks, which `privacy/TC-003` then had to step around.
- The app's live transcriber has the transcript ready about a second after `recording_stopped` (13 s after the hang-up in the 2026-11-08 evening run): in S-17 it is usually there before this stage begins, on a page that has not been reloaded since the call was Recording — the refresh is what is checked.
- Original step 2 (scroll position and selected text of the other call) is not scripted: Initech planning has two turns, nothing to scroll. The tab check stands in for it.
- Run on 2026-11-07 (S-17, TestCall and `capture`): PASS.
