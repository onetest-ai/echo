---
id: TC-007
title: Calls with no call chosen says Pick a call
priority: low
type: functional
requirements: [F-CALLS-04]
module: calls
run: agent
platform: any
tags: [calls, empty]
---

# TC-007: Calls with no call chosen says Pick a call

**Module:** Calls | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Route | `#calls` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Load `{{base_url}}/` and then navigate to `{{base_url}}/#calls` | The Calls list is on the left |
| 2 | Read the main area | The heading "Pick a call" and the text "Its minutes, your notes, the transcript, chat and shared screens open here."; no call header |

## Expected Final State
No call is open; the main area says Pick a call.

## Notes
- Creates or changes nothing. "60 at a time" and "No calls yet" need a larger or empty data set.
