---
id: TC-020
title: Four threads on the default five columns show the column counts of their tasks and questions
priority: medium
type: functional
requirements: [F-WORK-04]
module: work
run: agent
platform: any
tags: [work, board, columns, pairwise]
---

# TC-020: Four threads on the default five columns show the column counts of their tasks and questions

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- None of the four threads has saved board settings; each shows the view "Board" with no filter

## Test Data

| Thread | To do | In progress | Waiting | Blocked | Done |
|---|---|---|---|---|---|
| Acme renewal | 8 | 1 | 1 | 2 | 3 |
| Globex pilot | 6 | 1 | 1 | 3 | 1 |
| Initech hiring | 7 | 2 | 0 | 0 | 3 |
| Umbrella proposal | 6 | 1 | 1 | 2 | 2 |

Cards in the columns other than To do:

| Thread | Column | Cards |
|---|---|---|
| Acme renewal | Waiting | T003 |
| Acme renewal | Blocked | T011, T017 |
| Acme renewal | Done | T001, T002, T016 |
| Acme renewal | In progress | Q001 |
| Umbrella proposal | Waiting | T012 |
| Umbrella proposal | Blocked | T005, T010 |
| Umbrella proposal | Done | T001, T015 |
| Umbrella proposal | In progress | T013 |
| Initech hiring | Waiting | - |
| Initech hiring | Blocked | - |
| Initech hiring | Done | T002, T007, T011 |
| Initech hiring | In progress | T008, T012 |

Not on the board (closed in the ledger):

| Thread | Id | Title | State |
|---|---|---|---|
| Acme renewal | T004 | Share the architecture diagram | merged |
| Acme renewal | T007 | Write the cut-over runbook | cancelled |
| Acme renewal | T009 | Fix the SSO redirect | deleted |
| Acme renewal | T013 | Review the invoice | deleted |
| Acme renewal | T015 | Confirm the rollback window | deleted |
| Acme renewal | T020 | Rehearse the demo (2) | cancelled |
| Initech hiring | T001 | Update the risk log | cancelled |
| Initech hiring | T004 | Agree the support hours | merged |
| Initech hiring | T014 | Update the contact list (2) | merged |
| Umbrella proposal | T003 | Draft the status report | merged |
| Umbrella proposal | T006 | Ask for the budget code | cancelled |
| Umbrella proposal | T007 | Archive the old tenant | merged |
| Umbrella proposal | T016 | Sign off the data mapping (2) | deleted |

A To do card to look at: Initech hiring T010 "Prepare the handover pack" (open, owner Sam Rivera).

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-renewal/work`, click the view "Board" if it is not the active one, and read the five headers | "To do", "In progress", "Waiting", "Blocked", "Done" with the counts of Acme renewal's row of the first table |
| 2 | Read the cards of the columns other than To do, and look for Acme renewal's cards of the third table | The cards of Acme renewal's rows of the second table, and no others, are under those headers; T003 "Send the revised timeline" (in progress, waiting for Rita Gomez) is in "Waiting", not in "In progress"; none of the third table's cards is on the board (cancelled, merged and deleted tasks are not cards) |
| 3 | Navigate to `{{base_url}}/#project/globex-pilot/work`, click "Board" if it is not active, and read the five headers | The counts of Globex pilot's row |
| 4 | Navigate to `{{base_url}}/#project/initech-hiring/work`, click "Board" if it is not active, and read the headers, the cards of the columns other than To do and the cards of the third table | The counts of Initech hiring's row; the cards of its rows of the second table and no others (no card in Waiting or Blocked); Initech hiring T010 is in "To do"; none of its third-table cards is on the board |
| 5 | Navigate to `{{base_url}}/#project/umbrella-proposal/work`, click "Board" if it is not active, and read the headers, the cards of the columns other than To do and the cards of the third table | The counts of Umbrella proposal's row; the cards of its rows of the second table and no others; T015 "Check the licence count (2)" (done, a session) is in "Done"; none of its third-table cards is on the board |

## Expected Final State
Each board matches its row.

## Teardown
- None: the case only reads

## Notes
- Counts are of tasks and questions that are not cancelled, merged or deleted (a done one counts in Done even with "Open only" on).
