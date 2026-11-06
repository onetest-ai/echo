---
id: TC-031
title: A call whose automatic minutes are due now says Echo is starting them and offers no button
priority: medium
type: functional
requirements: [F-MIN-07]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, auto]
---

# TC-031: A call whose automatic minutes are due now says Echo is starting them and offers no button

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)
- Only `window --serve` runs: nothing starts the due minutes, so the state stays (the Echo app would start a run, which the fake claude refuses)

## Test Data

| Call | State |
|---|---|
| Initech planning | `auto.due` now, no run |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Initech planning, then the tab "Minutes" | A box with the heading "Starting the minutes…" and the text "Echo is starting them now." |
| 2 | Look for buttons in the box | No "Write minutes", no "Write now", no "Not yet" |

## Expected Final State
Initech planning says Echo is starting its minutes.

## Notes
- Creates or changes nothing.
