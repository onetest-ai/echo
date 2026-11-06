---
id: TC-004
title: A time chip in the minutes jumps to that moment of the transcript
priority: high
type: functional
requirements: [F-MIN-03]
module: window
run: agent
platform: any
tags: [calls, minutes, transcript]
external_id: TC-WIN-003
---

# TC-004: A time chip in the minutes jumps to that moment of the transcript

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync |
| Chip | 00:37 (in Summary) |
| Turn at 0:37 | Then we agree: pilot starts with the Berlin office. |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync" | Its minutes are shown |
| 2 | Click the time chip `00:37` after the first Summary line | The Transcript tab opens |
| 3 | Find the highlighted turn | The turn at 0:37, "Then we agree: pilot starts with the Berlin office.", is highlighted and in view |

## Expected Final State
The Transcript tab is open with the turn at 0:37 highlighted.
