---
id: TC-007
title: Shared-screen snapshots are opt-in and ask for Screen Recording
priority: medium
type: functional
requirements: [F-SET-08]
module: permissions
run: agent
platform: macos
tags: [permissions, frames]
external_id: TC-PERM-007
user_steps:
  - 'the user confirms the "Screen Recording needed" notification when the run asks'
---

# TC-007: Shared-screen snapshots are opt-in and ask for Screen Recording

**Module:** Permissions | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo installed and running, without full Screen Recording; `E=/Applications/Echo.app/Contents/MacOS/echoctl`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Settings → Recording in the window (served: app_profile.md → Clean-install runs) → tick "Save snapshots of shared screens" (off by default) | Saved in the config (`$E config` shows it on); a "Screen Recording needed" notification (the user confirms it) |

## Expected Final State
Snapshots are opt-in and say what they need.

## Notes
- Granting Screen Recording and the call with shared slides is recording/TC-022 (S-18).
