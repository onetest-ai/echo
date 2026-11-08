---
id: TC-005
title: A permission granted later is picked up
priority: high
type: functional
requirements: [F-SET-17]
module: permissions
run: agent
platform: macos
tags: [permissions]
external_id: TC-PERM-005
user_steps:
  - 'the user enables Echo under System Audio Recording in System Settings when the run asks'
---

# TC-005: A permission granted later is picked up

**Module:** Permissions | **Priority:** High | **Type:** Functional

## Preconditions
- After TC-003 (system audio denied); `E=/Applications/Echo.app/Contents/MacOS/echoctl`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** enable Echo under Privacy & Security → Screen & System Audio Recording when the run asks | — |
| 2 | **The user does:** Echo's own window → Settings → Health → "Re-check permissions"; the run: `$E doctor --json` | System Audio Recording granted in Health and in doctor without reinstalling (or Echo says to restart, if macOS needs it): Health ✓ when nothing else is missing (in S-16 the microphone stays denied for `permissions/TC-004`, so Health ⚠ names only the microphone) |

## Expected Final State
Fixing a permission needs no reinstall.
