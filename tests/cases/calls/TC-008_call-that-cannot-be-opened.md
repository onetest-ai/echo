---
id: TC-008
title: A call that cannot be read says This call can't be opened
priority: medium
type: functional
requirements: [F-CALLS-16]
module: calls
run: agent
platform: any
tags: [calls, error]
---

# TC-008: A call that cannot be read says This call can't be opened

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Route | `#call/2026-01-01_000000-no-such-call/minutes` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Load `{{base_url}}/` and then navigate to `{{base_url}}/#call/2026-01-01_000000-no-such-call/minutes` | The main area shows the heading "This call can't be opened" and an error text (for example "Error: 404") |
| 2 | Click "Calls" in the activity bar and then "Acme weekly sync" | The call opens normally on Minutes |

## Expected Final State
The error was shown for the missing folder and a real call opens afterwards.

## Notes
- Creates or changes nothing. A folder name of the data differs on every run: this case uses a name that never exists.
