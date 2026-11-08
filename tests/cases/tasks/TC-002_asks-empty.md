---
id: TC-002
title: Asks says nobody owes anything when nothing is late, due soon or quiet
priority: medium
type: functional
requirements: [F-TASK-08]
module: tasks
run: agent
platform: any
tags: [tasks, asks]
---

# TC-002: Asks says nobody owes anything when nothing is late, due soon or quiet

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- No change has been made to the tasks on the data (nothing is handed over by Sam)

## Test Data

| Field | Value |
|-------|-------|
| Panel | Asks (right panel, only while Tasks is open) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | The Tasks view is open |
| 2 | Click the top bar's button "Inbox" to open the right panel, then its tab "Asks" (`[data-panel-show="asks"]`) | The panel shows "Asks · by person" and "next time you meet" |
| 3 | Read the panel body | The text "Nothing to chase: nobody owes you anything late, due soon or gone quiet." and the footer "Computed from what went quiet, what's due and what's owed — never a model's guess."; no person name is listed |

## Expected Final State
The Asks tab shows its empty text and its footer; no person is listed.

## Teardown
- Close the panel (`[aria-label="Close the panel"]`)

## Notes
- Creates or changes nothing.
