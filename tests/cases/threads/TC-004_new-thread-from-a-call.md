---
id: TC-004
title: A call's "+ Thread" → "New thread from this call…" makes a thread named after the call with the call pinned
priority: medium
type: functional
requirements: [F-THR-05]
module: threads
run: agent
platform: any
tags: [threads, create, calls]
---

# TC-004: A call's "+ Thread" → "New thread from this call…" makes a thread named after the call with the call pinned

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- No thread `initech-planning` exists
- The call "Initech planning" is in no thread

## Test Data

| Field | Value |
|-------|-------|
| Call | Initech planning (row `[data-call$="initech-planning"]`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click the row "Initech planning" | The call opens; its header shows a button "+ Thread" and no thread chip |
| 2 | Click "+ Thread" | A menu opens listing "Acme rollout" and "Globex pilot" and the item "New thread from this call…" |
| 3 | Click "New thread from this call…". Wait up to 3 s after an action for the window to redraw before reading. | The URL ends with `#project/initech-planning/profile`; the heading "Initech planning" |
| 4 | Read the Calls row of the Profile | A token `series “Initech planning”` and the line "1 call match" |
| 5 | Click the tab with `data-tab-id="calls"` | A row "Initech planning" with "added by hand" and a button "Unpin" |

## Expected Final State
Thread Initech planning exists with the call Initech planning pinned in it.

## Teardown
- `echoctl project delete initech-planning`

## Changes
- Creates thread `initech-planning`, pins the call Initech planning, adds a series rule.

## Notes
Calendar cards ("+ Thread" there) are not covered by this case; the same code path (`event-new`) applies.
