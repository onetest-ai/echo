---
id: TC-013
title: A very short call gets a transcript and no automatic minutes
priority: low
type: functional
module: recording
run: manual
platform: macos
tags: [recording, minutes]
external_id: TC-REC-013
---

# TC-013: A very short call gets a transcript and no automatic minutes

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Arm, then make a 30-second call | A transcript is made |
| 2 | Open its Minutes | "No minutes written automatically" (too short); no error |

## Expected Final State
Short calls are kept without errors or automatic minutes.
