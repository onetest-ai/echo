---
id: TC-005
title: A permission granted later is picked up
priority: high
type: functional
module: permissions
run: manual
platform: macos
tags: [permissions]
external_id: TC-PERM-005
---

# TC-005: A permission granted later is picked up

**Module:** Permissions | **Priority:** High | **Type:** Functional

## Preconditions
- After TC-003

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Enable Echo under System Audio Recording in System Settings | — |
| 2 | Menu → Health → "Re-check permissions" | Health ✓ without reinstalling (or Echo says to restart, if macOS needs it) |

## Expected Final State
Fixing a permission needs no reinstall.
