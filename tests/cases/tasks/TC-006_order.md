---
id: TC-006
title: Order sorts by due date with late first, by quiet time, or by thread
priority: medium
type: functional
requirements: [F-TASK-03]
module: tasks
run: agent
platform: any
tags: [tasks, order]
---

# TC-006: Order sorts by due date with late first, by quiet time, or by thread

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Globex pilot's T001 is overdue: `echoctl project ledger globex-pilot edit '{"op":"edit","node":"T001","set":{"due":"<the date two days ago, YYYY-MM-DD>"}}'`. Every `echoctl` command runs after `. <dir>/env` (the run's env file)
- In S-08 it is already overdue: stage 5 (tasks/TC-005) set it and kept it (its Teardown skipped). On its own: run the edit above

## Test Data

| Field | Value |
|-------|-------|
| Late task | Draft the Globex pilot plan (Globex pilot) |
| Due today | Send Acme the SSO checklist (Acme rollout) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | The "Order" chip reads "Due ▾" |
| 2 | Read the cards of "I owe" | "Draft the Globex pilot plan" first (overdue), then "Send Acme the SSO checklist" |
| 3 | Select "Thread" in `[aria-label="Order"]` (`browser_select_option`) | The chip reads "Thread ▾"; "I owe" lists "Send Acme the SSO checklist" (Acme rollout) first, then "Draft the Globex pilot plan" (Globex pilot) |
| 4 | Select "Quiet longest" | The chip reads "Quiet longest ▾"; "I owe 2" still holds the same two cards (neither has been quiet: their order is not asserted) |
| 5 | Select "Due" | The order of step 2 returns |

## Expected Final State
Order is back on Due with the overdue task first; no task was added or lost by any order.

## Teardown
- `echoctl project ledger globex-pilot edit '{"op":"edit","node":"T001","set":{"due":null}}'` (after `. <dir>/env`) puts the task back to no date

## Notes
- Changes Globex pilot T001's due date (undone in Teardown).
