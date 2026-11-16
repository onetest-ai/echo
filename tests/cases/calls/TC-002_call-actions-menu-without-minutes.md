---
id: TC-002
title: Call actions leaves out the minutes items for a call without minutes
priority: medium
type: functional
requirements: [F-CALLS-11]
module: calls
run: agent
platform: any
tags: [calls, menu]
auto: [tests/auto/specs/S-09.spec.mjs]
---

# TC-002: Call actions leaves out the minutes items for a call without minutes

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Initech planning (a transcript, no minutes) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Initech planning" | The call opens |
| 2 | Click "Call actions" | A menu opens |
| 3 | Read the items | Exactly "Show folder", "Ask Claude about this call" and "Delete…"; no "Open minutes in browser", "Write minutes again", "Write again with" or "Edit templates…" |
| 4 | Press Escape | The menu closes |

## Expected Final State
The menu is closed; the call is open.

## Notes
- Creates or changes nothing.
