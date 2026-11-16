---
id: TC-003
title: System audio denied is flagged, not silently recorded
priority: critical
type: functional
requirements: [F-SET-17]
module: permissions
run: agent
platform: macos
tags: [permissions, capture]
external_id: TC-PERM-003
user_steps:
  - 'the user runs `tccutil reset All ai.onetest.echo` and restarts Echo when the run asks'
  - 'at the System Audio Recording prompt the user clicks Don''t Allow when the run asks'
---

# TC-003: System audio denied is flagged, not silently recorded

**Module:** Permissions | **Priority:** Critical | **Type:** Functional

## Preconditions
- Echo installed and running; `E=/Applications/Echo.app/Contents/MacOS/echoctl`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** `tccutil reset All ai.onetest.echo`, quit and reopen Echo; at the System Audio Recording prompt click "Don't Allow" when the run asks (in S-16 at the Microphone prompt too: `permissions/TC-004`'s state) | — |
| 2 | `$E doctor --json`; the user reads Echo's own window → Settings → Health | Health ⚠ with system audio missing; "Ask for permissions…" / "Re-check permissions" say where to enable it |

## Expected Final State
A missing system-audio permission is visible before a call.

## Notes
- The call with system audio denied is recording/TC-020 (S-18).
