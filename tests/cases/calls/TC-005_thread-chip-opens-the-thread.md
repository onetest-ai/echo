---
id: TC-005
title: A thread chip opens its thread and says why the call is in it
priority: medium
type: functional
requirements: [F-CALLS-09]
module: calls
run: agent
platform: any
tags: [calls, threads]
auto: [tests/auto/specs/S-09.spec.mjs]
---

# TC-005: A thread chip opens its thread and says why the call is in it

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync |
| Thread | Acme rollout |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync" | The header shows a chip "Acme rollout" and the button "+ Thread" |
| 2 | Read the chip's tooltip | "added by hand" (the data pins the call) |
| 3 | Click the chip "Acme rollout" | The URL becomes `#project/acme-rollout/…` and the heading "Acme rollout" is shown |
| 4 | Open its tab "Calls" (`[data-pj="tab"][data-tab-id="calls"]`) | "Acme weekly sync" is listed |

## Expected Final State
The thread Acme rollout is open on its Calls tab with Acme weekly sync in it.

## Notes
- Creates or changes nothing. A suggestion bar "Part of X? Add / Not this" needs a call that matches a thread's rules without being in it: `standard` data set.
