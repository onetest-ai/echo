---
id: TC-002
title: Health shows everything granted
priority: critical
type: functional
requirements: [F-SET-17]
module: permissions
run: agent
platform: macos
tags: [permissions, health]
external_id: TC-PERM-002
---

# TC-002: Health shows everything granted

**Module:** Permissions | **Priority:** Critical | **Type:** Functional

## Preconditions
- Permissions granted (TC-001); `E=/Applications/Echo.app/Contents/MacOS/echoctl`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E doctor --json`; the user reads the tray menu's Health item and Echo's own window → Settings → Health when the run asks | doctor: every required permission granted, `ready` true; the user confirms "Health ✓" in the menu and ✓ beside each permission in the window |

## Expected Final State
Health reflects the granted permissions.
