---
id: TC-021
title: Microphone denied: the others are still recorded
priority: high
type: functional
requirements: [F-REC-15]
module: recording
run: person
platform: macos
tags: [recording, permissions]
external_id: TC-REC-021
---

# TC-021: Microphone denied: the others are still recorded

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed with Microphone denied and System Audio Recording allowed (permissions/TC-004)
- A Teams call to join (or TestCall with the debug build)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a short call | The others are recorded and transcribed; your side is empty and flagged |

## Expected Final State
One missing track doesn't lose the other.

## Notes
- Stays person-run: Microphone is denied in System Settings (by the user, S-16). With it denied, a TestCall call would do for the call itself.
- Moved from permissions/TC-004 step 2.
