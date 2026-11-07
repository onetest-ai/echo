---
id: TC-020
title: System audio denied: the call flags the others' side as missing
priority: high
type: functional
requirements: [F-REC-15]
module: recording
run: person
platform: macos
tags: [recording, permissions]
external_id: TC-REC-020
---

# TC-020: System audio denied: the call flags the others' side as missing

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed with System Audio Recording denied (permissions/TC-003)
- A Teams call to join (or TestCall with the debug build, app_profile.md → Test calls)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a short call (Record next call, join, speak on both sides, hang up) | The others' side is flagged as missing (a notice in the call, Health ⚠), never an empty track without a warning |

## Expected Final State
A call recorded without system audio says so.

## Notes
- Moved from permissions/TC-003 step 3.
