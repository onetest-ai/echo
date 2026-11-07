---
id: TC-018
title: On the default board a card lands in the column of its state, a blocked one in Blocked even if it waits
priority: high
type: functional
requirements: [F-WORK-04, F-WORK-02]
module: work
run: agent
platform: any
tags: [work, board, columns, pairwise]
---

# TC-018: On the default board a card lands in the column of its state, a blocked one in Blocked even if it waits

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- Globex pilot has no saved board settings (the default five columns); its view is Board and no filter is set

## Test Data

| Id | Kind | Title | State | Waiting for | Column |
|---|---|---|---|---|---|
| T002 | task | Unblock: collect the user list | open | - | To do |
| T006 | task | Schedule the steering call | open | - | To do |
| T007 | task | Unblock: schedule the steering call | open | - | To do |
| T010 | task | Unblock: reporting dashboard | open | - | To do |
| T012 | task | Send the welcome mail (2) | open | - | To do |
| T014 | task | Review the access matrix (2) (kept) | open | - | To do |
| T009 | task | Send the welcome mail | in_progress | - | In progress |
| T005 | task | Book the training slots | open | Kim Park | Waiting |
| T001 | task | Collect the user list | blocked | - | Blocked |
| T004 | task | Check the firewall rules | blocked | Kim Park | Blocked |
| Q001 | question | When does finance decide? | blocked | - | Blocked |
| T008 | task | Collect feedback from Berlin | done | Kim Park | Done |

Not on the board (closed in the ledger): T003 deleted, T011 cancelled, T013 merged; D001 (decision) and R001 (risk) are not tasks or questions.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/work`, click the view "Board" if it is not the active one, wait for "Not synced" or "Sync failed" in the bar | The column headers are in this order with these counts: "To do" 6, "In progress" 1, "Waiting" 1, "Blocked" 3, "Done" 1, then "+ Column"; no "hidden" link |
| 2 | For each row of the table find the card `[data-pj="node"][data-node=<Id>]` and read the column header above it | Each card is in the column of its row; the question Q001 shows the kind chip "question" and no owner avatar |
| 3 | Look at T004 "Check the firewall rules" (state blocked, waiting for Kim Park) and T005 "Book the training slots" (open, waiting for Kim Park) | T004 is in "Blocked" and T005 in "Waiting": a blocked task stays in Blocked even when it waits for a person |
| 4 | Look at T008 "Collect feedback from Berlin" (done, waiting for Kim Park) | It is in "Done" |
| 5 | Look for the cards T003, T011, T013, D001 and R001 | None is on the board |

## Expected Final State
Globex pilot shows the board of the table.

## Teardown
- None: the case only reads

## Notes
- Order of precedence in `cardCategory`: done, blocked, waiting for someone, in progress, to do. The Tree reads the same blocked-and-waiting task as "waiting" (`TC-022`).
- Covers node.kind (task, question) by state by wait on the board.
