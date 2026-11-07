---
id: TC-019
title: A board with custom columns shows the limit exceeded, a hidden Waiting column and a card whose stored column does not fit
priority: medium
type: functional
requirements: [F-WORK-04, F-WORK-06]
module: work
run: agent
platform: any
tags: [work, board, columns, limit]
---

# TC-019: A board with custom columns shows the limit exceeded, a hidden Waiting column and a card whose stored column does not fit

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- Acme rollout's view "Board" is active, no filter is set, "Open only" is on

## Test Data

| Id | Kind | Title | State | Stored column | Shown in |
|---|---|---|---|---|---|
| Q003 | question | Is Munich in scope? | open | - | Backlog |
| T002 | task | Get the access list | open | - | Backlog |
| T008 | task | Run the data migration dry run | open | review | Backlog |
| T007 | task | Answer the security questionnaire | in_progress | - | In progress |
| T011 | task | Confirm the go-live date | in_progress | - | In progress |
| T016 | task | Map the user groups | in_progress | - | In progress |
| T023 | task | Send the meeting notes (2) | blocked | - | Blocked |
| Q001 | question | Who signs off the pilot on Acme's side? | done | - | Done |
| Q002 | question | Who owns the help desk queue? | done | - | Done |
| T001 | task | Send the SSO checklist | done | - | Done |
| T009 | task | Prepare the renewal quote | done | - | Done |
| T022 | task | Confirm the go-live date (2) | done | - | Done |

Columns of the project: Backlog (state to do, limit 3), In progress (limit 10), In review (state in progress), Waiting (hidden in the view "Board"), Blocked, Done.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work`, click the view "Board" | The headers are: "Backlog" 14 / 3, "In progress" 3 / 10, "In review" 0, "Blocked" 1, "Done" 5; "+ Column" and a link "1 hidden"; no header "Waiting" |
| 2 | Read the count beside "Backlog" | It reads "14 / 3" and is marked over the limit (class `over`) |
| 3 | For each row of the table find the card and read its column header | Each card is in the column of its row |
| 4 | Look at T008 "Run the data migration dry run", whose stored column is "In review" | It is in "Backlog", not in "In review": the stored column is a column of the state "in progress" and the task is to do, so the first column of its own state takes it |

## Expected Final State
Acme rollout shows the board of the first step.

## Teardown
- None: the case only reads

## Notes
- Does not press "1 hidden", which un-hides the column for good: `TC-006` covers the column menu.
- Covers view.columns (renamed, hidden, limit over) and node.column (a custom column of another state).
