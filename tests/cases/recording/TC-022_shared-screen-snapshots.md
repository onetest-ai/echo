---
id: TC-022
title: Shared-screen snapshots: one frame per slide change
priority: medium
type: functional
requirements: [F-REC-12]
module: recording
run: person
platform: macos
tags: [recording, permissions]
external_id: TC-REC-022
---

# TC-022: Shared-screen snapshots: one frame per slide change

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- "Save snapshots of shared screens" on (permissions/TC-007)
- A call where someone shares slides

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Allow full Screen Recording for Echo in System Settings | — |
| 2 | Record a call where someone shares slides and changes them | The call has a Screens tab with about one frame per slide change, not one every few seconds |

## Expected Final State
Snapshots are kept sparse.

## Notes
- Moved from permissions/TC-007 steps 2–3.
