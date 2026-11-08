---
id: TC-025
title: Asks lists what is due soon, quiet or waited for, by person, at most two per story
priority: medium
type: functional
requirements: [F-TASK-08, F-TASK-04]
module: tasks
run: agent
platform: any
tags: [tasks, asks, waiting, quiet, due]
---

# TC-025: Asks lists what is due soon, quiet or waited for, by person, at most two per story

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

| Person | Thread | Id | Button text | Reason after the dash |
|---|---|---|---|---|
| Alex Chen | Acme renewal | T011 | Test the password reset | due <the build date, e.g. "Sun, Nov 8"> |
| Alex Chen | Globex pilot | T005 | Book the training slots | due <the day after, e.g. "Mon, Nov 9"> |
| Alex Chen | Initech hiring | T013 | Unblock: agree the support hours (2) | waiting |
| Alex Chen | Acme renewal | T018 | Unblock: send the revised timeline (2) | waiting |
| Alex Chen | Globex pilot | T002 | Unblock: collect the user list | waiting |
| Zoe Martin | Umbrella proposal | T010 | Check the licence count | quiet 5 days |
| Zoe Martin | Umbrella proposal | T012 | an answer on draft the status report (2) | waiting · an answer on draft the status report (2) |

Not asked, on purpose: Rita Gomez (the wait card of Acme renewal T003), because Acme renewal's story "Day-to-day work" already has its two asks (at most two per story); Kim Park (no task of the user waits on a task owned by Kim).

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks`, wait for "I owe", click the top bar's "Inbox" to open the right panel and its tab "Asks" (`[data-panel-show="asks"]`) | The panel shows "Asks · by person" and "next time you meet" |
| 2 | Read the person blocks | Two blocks, in this order: "Alex Chen" and "Zoe Martin"; each has a line on the right with a day and a meeting name, or "no meeting this week" (not asserted); the empty text "Nothing to chase" is not shown |
| 3 | Read the buttons of each block | The 7 buttons of the table: 5 under Alex Chen, 2 under Zoe Martin, in that order, each reading the title, " — " and its reason; no reason is red (nothing is late) |
| 4 | Select "Acme renewal · 11" in `[aria-label="Thread"]` | The panel keeps "Alex Chen" with "Test the password reset" and "Unblock: send the revised timeline (2)" only; "Zoe Martin" is gone |
| 5 | Select "Umbrella lead · 0" | The panel says "Nothing to chase: nobody owes you anything late, due soon or gone quiet." |
| 6 | Select "All threads" and click the button "Check the licence count" under Zoe Martin | The tab "Details" opens on Umbrella proposal T010 "Check the licence count" with the path "General › Day-to-day work" (the workstream, then the story) |

## Expected Final State
The Details drawer shows Umbrella proposal T010; the Thread filter is on All threads.

## Teardown
- Close the panel (`[aria-label="Close the panel"]`)

## Notes
- Extends `TC-008` (one late task) with the three reasons that exist on the data: due within 3 days, quiet 5 days, waiting. No task of the data is late, so "was due" does not occur.
- Per-person order follows the most urgent ask first (late, then the nearest date, then the quietest).
- The reason of a dated task is `due <Day, Mon D>` (`fmtDay`), the date itself, not "today" / "tomorrow": the case once said the words, which the window never showed (2026-11-08 run).
