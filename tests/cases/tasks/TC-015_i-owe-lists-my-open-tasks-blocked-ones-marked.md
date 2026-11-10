---
id: TC-015
title: I owe holds the user's open tasks, a blocked one stays there marked and says by whom
priority: high
type: functional
requirements: [F-TASK-01, F-TASK-02, F-TASK-06]
module: tasks
run: agent
platform: any
tags: [tasks, piles, pairwise, blocked]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-015: I owe holds the user's open tasks, a blocked one stays there marked and says by whom

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

| Thread | Id | Title | Line under the title | Due (relative to the build) | Mode icon |
|---|---|---|---|---|---|
| Acme rollout | T010 | Book the workshop room | Day-to-day work | 3 days ago (late) | - |
| Acme rollout | T016 | Map the user groups | Day-to-day work | 3 days ago (late) | You do it by hand |
| Acme renewal | T003 | Send the revised timeline | Day-to-day work · blocked by Rita Gomez | today | Claude can do it |
| Globex pilot | T001 | Collect the user list | Day-to-day work · blocked by T002 | tomorrow | You do it by hand |
| Umbrella proposal | T009 | Close the open tickets | Day-to-day work | in 20 days | Claude can do it |
| Umbrella proposal | T005 | Prepare the demo script | Day-to-day work · blocked | no date | Claude can do it |
| Umbrella proposal | T012 | Draft the status report (2) | Day-to-day work · blocked by Zoe Martin | no date | - |
| Umbrella proposal | T013 | Prepare the demo script (2) | Day-to-day work | no date | - |
| Initech hiring | T010 | Prepare the handover pack | Day-to-day work | no date | You do it by hand |
| Initech hiring | T012 | Agree the support hours (2) | Day-to-day work · blocked by T013 | no date | - |
| Acme renewal | T017 | Send the revised timeline (2) | Day-to-day work · blocked by T018 | no date | - |
| Acme renewal | T019 | Share the architecture diagram (2) | Day-to-day work | no date | - |
| Acme rollout | T023 | Send the meeting notes (2) | Day-to-day work · blocked | no date | - |
| Globex pilot | T012 | Send the welcome mail (2) | Day-to-day work | no date | - |

The due chip shows the date itself, as the browser's locale writes it: with a short weekday for today and the next six days ("Tue, Nov 10", "Wed, Nov 11" in en-US), day and short month otherwise ("Nov 7", "Nov 30"); a late one is red (`tk-due late`), "no date" when there is none (`TC-020`). The mode icon's `aria-label` starts with the words of the column: a task done by hand adds how (`You do it by hand: <how>`).

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | The headings read "I owe 14", "Owed to me 5", "Waiting for others 5", "Nobody named 11" and the folded button "Others · 24"; the Thread chip reads "All threads ▾" and the header says "across 7 active projects" |
| 2 | Click the button "+ 8" at the foot of the "I owe" column | The button now reads "Show fewer"; the column holds 14 cards |
| 3 | For each row of the table find the card `[data-tk="open"][data-project=<thread id>][data-node=<Id>]` in the "I owe" column (thread ids: acme-rollout, acme-renewal, globex-pilot, initech-hiring, umbrella-proposal) and read it | Every card exists in the column and shows the title, the thread name, the line under the title, the due chip of its row's date and the mode icon of its row (its `aria-label` starts with the row's words); no card of the column is missing from the table |
| 4 | Count the cards of the column whose line under the title holds the red word "blocked" | 7 cards (the rows whose line says "blocked"); the other 7 show only the story "Day-to-day work" |
| 5 | Run `echoctl project tasks` and compare `piles.mine` | 14 entries with the same thread and id pairs; `blocked` is true on exactly the 7 blocked ones and `mine` is true on all 14 |

## Expected Final State
The 14 cards of the table are in I owe; the pile is left unfolded.

## Teardown
- None: the case only reads

## Notes
- The user is the owner "Sam Rivera" or "Me"; the data also holds closed tasks of the user (done, cancelled, merged, deleted) that are in no pile: `TC-019`.
- A blocked task of the user stays in I owe: it is blocked by a person it waits on (`waiting_for`), by an open task it waits for (`after`), or by its own state.
