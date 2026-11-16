---
id: TC-003
title: The Thread filter narrows every pile and shows the count of each thread
priority: medium
type: functional
requirements: [F-TASK-04]
module: tasks
run: agent
platform: any
tags: [tasks, filters]
auto: [tests/auto/specs/S-08.spec.mjs]
---

# TC-003: The Thread filter narrows every pile and shows the count of each thread

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- No change has been made to the tasks on the data

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "I owe 2" and the button "Others · 1" |
| 2 | Open the "Thread" filter (`[aria-label="Thread"]`) | Its options are "All threads", "Acme rollout · 2", "Globex pilot · 1" |
| 3 | Select "Globex pilot · 1" (`browser_select_option`) | The chip reads "Globex pilot ▾"; "I owe 1" with only the card "Draft the Globex pilot plan"; the "Others" button is gone |
| 4 | Select "Acme rollout · 2" | "I owe 1" with "Send Acme the SSO checklist"; the button "Others · 1" is back |
| 5 | Select "All threads" | "I owe 2" and "Others · 1" as at the start |

## Expected Final State
The filter is back on All threads; both I owe cards and Others · 1 are shown.

## Notes
- The filter is in the page only; nothing is written. A reload also resets it.
