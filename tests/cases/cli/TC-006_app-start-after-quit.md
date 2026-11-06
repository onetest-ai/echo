---
id: TC-006
title: echoctl app start brings Echo back after a quit
priority: low
type: functional
requirements: [F-LIFE-03, F-CLI-03]
module: cli
run: person
platform: macos
tags: [cli, lifecycle]
external_id: TC-CLI-004
---

# TC-006: echoctl app start brings Echo back after a quit

**Module:** CLI | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar → "Quit Echo" | The icon goes |
| 2 | Run `echoctl status` | `watcher` is null: the app isn't running |
| 3 | Run `echoctl app start` | Echo starts; the menu-bar icon is back |

## Expected Final State
With the app quit, status says so and `app start` starts it.
