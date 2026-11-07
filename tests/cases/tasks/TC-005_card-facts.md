---
id: TC-005
title: A card shows its story, thread and due date, a late one marked, and no date when it has none
priority: high
type: functional
requirements: [F-TASK-06]
module: tasks
run: agent
platform: any
tags: [tasks, cards]
---

# TC-005: A card shows its story, thread and due date, a late one marked, and no date when it has none

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Globex pilot's T001 is overdue: `echoctl project ledger globex-pilot edit '{"op":"edit","node":"T001","set":{"due":"<the date two days ago, YYYY-MM-DD>"}}'`. Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Late task | Draft the Globex pilot plan (due two days ago) |
| Due today | Send Acme the SSO checklist |
| No date | Answer the security questionnaire (in Others) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/` fully (`location.reload()`; a hash-only Navigate keeps the old data and shows "no date" for T001), navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "I owe 2" |
| 2 | Read the card "Draft the Globex pilot plan" | Under the title the story "Pilot plan"; the thread "Globex pilot"; a date whose tooltip is "Overdue" (`.tk-due.late`) |
| 3 | Read the card "Send Acme the SSO checklist" | The story "SSO for the Berlin pilot"; the thread "Acme rollout"; a date whose tooltip is "Due" and which is not marked late |
| 4 | Click "Others · 1" and read the card "Answer the security questionnaire" | The owner "Omar Haddad"; the thread "Acme rollout"; the text "no date" |
| 5 | Look for a quiet mark on every card | None shows "quiet … d" (no task is 5 days old) |

## Expected Final State
Each card shows its story or owner, its thread and its due state: late, due, or no date.

## Teardown
- `echoctl project ledger globex-pilot edit '{"op":"edit","node":"T001","set":{"due":null}}'` (after `. <dir>/env`) puts the task back to no date

## Notes
- Changes Globex pilot T001's due date (undone in Teardown). The date text differs per run; assert the tooltip (Overdue / Due) and "no date".
