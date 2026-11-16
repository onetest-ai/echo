---
id: TC-007
title: A task handed over by Sam goes to Owed to me and leaves Others
priority: high
type: functional
requirements: [F-TASK-02]
module: tasks
run: agent
platform: any
tags: [tasks, piles]
auto: [tests/auto/specs/S-08.spec.mjs]
---

# TC-007: A task handed over by Sam goes to Owed to me and leaves Others

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Omar's task is handed over by Sam and late: `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T002","set":{"delegated_by":"Me","due":"<the date two days ago, YYYY-MM-DD>"}}'`. Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Task | Answer the security questionnaire (Omar Haddad, Acme rollout, T002) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "I owe 2" |
| 2 | Read the pile headings | "Owed to me 1"; "Waiting for others 0"; "Nobody named 0" |
| 3 | Read the card under the heading "Owed to me 1" (the column whose heading reads "Owed to me") | One card: "Answer the security questionnaire", the owner "Omar Haddad", the thread "Acme rollout", a date marked "Overdue" |
| 4 | Look below the piles | No "Others" button (the task left that pile) |
| 5 | Run `echoctl project tasks` | `piles.owed` holds "Answer the security questionnaire" with `late: true`; `piles.others` is empty |

## Expected Final State
The task is in Owed to me only; Others is gone.

## Teardown
- `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T002","set":{"delegated_by":null,"due":null}}'` puts Omar's task back in Others

## Notes
- Changes Acme rollout T002 (undone in Teardown).
