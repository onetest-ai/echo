---
id: TC-004
title: Microphone denied is flagged in Health
priority: high
type: functional
requirements: [F-SET-17]
module: permissions
run: agent
platform: macos
tags: [permissions, capture]
external_id: TC-PERM-004
user_steps:
  - 'the user runs `tccutil reset All ai.onetest.echo` and restarts Echo when the run asks'
  - 'the user clicks Don''t Allow for Microphone and Allow for System Audio Recording when the run asks'
---

# TC-004: Microphone denied is flagged in Health

**Module:** Permissions | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed and running; `E=/Applications/Echo.app/Contents/MacOS/echoctl`
- **Microphone denied, System Audio Recording granted (shared in S-16):** in S-16 from `permissions/TC-003`'s reset (both prompts denied) and `permissions/TC-005` (system audio granted later), so step 1 is skipped. On its own: step 1

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** `tccutil reset All ai.onetest.echo`, quit and reopen Echo; deny Microphone, allow System Audio Recording when the run asks | — |
| 2 | `$E doctor --json`; the user reads Echo's own window → Settings → Health | Health ⚠ shows the microphone missing, system audio granted |

## Expected Final State
A missing microphone permission is visible before a call.

## Notes
- The call with the microphone denied is recording/TC-021 (S-18).
