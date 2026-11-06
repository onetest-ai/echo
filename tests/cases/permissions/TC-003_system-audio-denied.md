---
id: TC-003
title: System audio denied is flagged, not silently recorded
priority: critical
type: functional
requirements: [F-SET-17, F-REC-15]
module: permissions
run: manual
platform: macos
tags: [permissions, capture]
external_id: TC-PERM-003
---

# TC-003: System audio denied is flagged, not silently recorded

**Module:** Permissions | **Priority:** Critical | **Type:** Functional

## Preconditions
- `tccutil reset All ai.onetest.echo`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At the System Audio Recording prompt click "Don't Allow" | — |
| 2 | Open the menu | "Health ⚠" with system audio missing; "Ask for permissions…" / "Re-check permissions" say where to enable it |
| 3 | Record a short Teams call | The remote side is flagged as missing, never an empty track without a warning |

## Expected Final State
A missing system-audio permission is visible before and during a call.
