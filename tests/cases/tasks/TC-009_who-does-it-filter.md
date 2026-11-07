---
id: TC-009
title: Who does it filters the piles after a task is set to By hand
priority: medium
type: functional
requirements: [F-TASK-04]
module: tasks
run: agent
platform: any
tags: [tasks, filters, mode]
---

# TC-009: Who does it filters the piles after a task is set to By hand

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- No task has a mode on the data (`mode: null` in `echoctl project tasks`)

## Test Data

| Field | Value |
|-------|-------|
| Task | Draft the Globex pilot plan |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "I owe 2" |
| 2 | Select "By hand" in `select[aria-label="Who does it"]` (the Details also has a button group with that label, so scope to `select`) | The chip reads "By hand ▾"; every pile shows "Nothing here" (no task has a mode yet) |
| 3 | Select "Anyone", click the card "Draft the Globex pilot plan", and in the Details press "By hand" (`[data-mode-set="manual"]`) | The button "By hand" is pressed (`aria-pressed="true"`); a row "How" appears with "not said" |
| 4 | Select "By hand" in "Who does it" | "I owe 1" with only "Draft the Globex pilot plan", which shows a hand icon (aria-label "You do it by hand"); "Send Acme the SSO checklist" is not shown |
| 5 | Select "Claude can do" | Every pile shows "Nothing here" |
| 6 | Select "Anyone" | "I owe 2" again |

## Expected Final State
The filter is on Anyone; Draft the Globex pilot plan carries a By hand mark.

## Teardown
- In the Details, History, press "undo" next to "you set mode" (or `echoctl project ledger globex-pilot edit '{"op":"edit","node":"T001","set":{"mode":null}}'`)
- Close the panel

## Notes
- Writes the choice onto the task (a human edit, kept). Uses no Claude Code.
