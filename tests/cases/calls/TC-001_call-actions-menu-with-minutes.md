---
id: TC-001
title: Call actions lists the minutes items for a call that has minutes
priority: high
type: functional
requirements: [F-CALLS-11]
module: calls
run: agent
platform: any
tags: [calls, menu]
---

# TC-001: Call actions lists the minutes items for a call that has minutes

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync" | The call opens on Minutes |
| 2 | Click "Call actions" (`[aria-label="Call actions"]`) | A menu (`role=menu`) opens |
| 3 | Read the items | "Show folder", "Open minutes in browser", "Show minutes in Finder" ("Show minutes in Explorer" on Windows), "Write minutes again", the label "Write again with" followed by "Default" and "Discussion" (not the template these minutes already use) and "Edit templates…", "Ask Claude about this call", and "Delete…" |
| 4 | Press Escape | The menu closes; the call is still open |

## Expected Final State
The menu is closed with the call open; no item was pressed.

## Notes
- Never press an item: Write minutes again and Ask Claude start Claude Code; the folder and browser items open the real machine's apps (manual, window/TC-012). Creates or changes nothing.
