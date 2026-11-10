---
id: TC-013
title: Only a call with automatic minutes ahead carries the clock chip in the list
priority: medium
type: functional
requirements: [F-MIN-07, F-CALLS-02]
module: calls
run: agent
platform: any
data: standard
tags: [calls, list, minutes]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-013: Only a call with automatic minutes ahead carries the clock chip in the list

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)

## Test Data

| Row | Chip |
|---|---|
| Globex standup (`globex-standup`) | `.mn-clock` "⏱ N min" with N from 1 to 15, title "Minutes at HH:MM" |
| Initech planning, Acme weekly sync (Today), Quick check-in, Umbrella contract call (`umbrella-contract-call`) | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" | The list is shown |
| 2 | Read the `.mn-clock` inside the row Globex standup | Its text starts with "⏱" and ends with "min", the number N is between 1 and 15; its `title` is "Minutes at HH:MM" |
| 3 | Look for `.mn-clock` inside the rows Initech planning, Acme weekly sync, Quick check-in and Umbrella contract call | None of the four has one |
| 4 | Count `.mn-clock` in the whole list | Exactly 1 |

## Expected Final State
Exactly one row, Globex standup, shows the clock chip.

## Notes
- Creates or changes nothing. Initech planning is due "now" and has no chip (its minutes are not ahead).
