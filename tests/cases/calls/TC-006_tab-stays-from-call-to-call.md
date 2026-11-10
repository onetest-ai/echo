---
id: TC-006
title: The Minutes and Transcript tabs are the only ones, and the chosen tab stays for the next call
priority: medium
type: functional
requirements: [F-CALLS-06, F-CALLS-07]
module: calls
run: agent
platform: any
tags: [calls, tabs]
auto: [tests/auto/specs/S-09.spec.mjs]
---

# TC-006: The Minutes and Transcript tabs are the only ones, and the chosen tab stays for the next call

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Calls | Acme weekly sync, Initech planning |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync" | A call may already be open (a full load opens the newest; a hash change keeps the last one open: app_profile.md, Calls); after the click Acme weekly sync opens on "Minutes" (`aria-selected="true"`); the tabs are "Minutes" and "Transcript" only (no Prep, Chat or Screens) |
| 2 | Click the tab "Transcript" | The URL ends `/transcript`; its five turns are shown |
| 3 | Click "Initech planning" in the list | It opens on "Transcript" (the last tab stays); two turns are shown |
| 4 | Click the tab "Minutes" | "No minutes yet" is shown |
| 5 | Click "Acme weekly sync" | It opens on "Minutes" |

## Expected Final State
Acme weekly sync is open on Minutes.

## Notes
- Per the code (`S.tab` in `doc()`): the tab is kept from one call to the next, and a call with only a transcript is not moved to Transcript by itself (only a call recording or being transcribed is). The map's F-CALLS-06 says "else on Transcript"; this case follows the code.
- Creates or changes nothing.
