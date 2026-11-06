---
id: TC-002
title: Health shows everything granted
priority: critical
type: functional
requirements: [F-SET-17]
module: permissions
run: manual
platform: macos
tags: [permissions, health]
external_id: TC-PERM-002
---

# TC-002: Health shows everything granted

**Module:** Permissions | **Priority:** Critical | **Type:** Functional

## Preconditions
- Permissions granted (TC-001)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the menu | "Health ✓"; inside, each permission granted |

## Expected Final State
Health reflects the granted permissions.
