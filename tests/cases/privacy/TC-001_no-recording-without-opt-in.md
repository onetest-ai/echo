---
id: TC-001
title: Nothing records without opting in
priority: critical
type: functional
requirements: [F-PRIV-01, F-REC-03]
module: privacy
run: manual
platform: any
tags: [privacy, opt-in]
external_id: TC-PRIV-001
---

# TC-001: Nothing records without opting in

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- Echo idle: not armed, nothing scheduled, no series set to record always

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Join three Teams calls during a day | The menu-bar icon stays idle in every call |
| 2 | Check the Calls list | No new call |
| 3 | Run `find <data folder> -name '*.wav'` | Nothing |

## Expected Final State
Without the user's opt-in no call is recorded and no audio exists.
