---
id: TC-003
title: Quit leaves no process and restarts normally
priority: high
type: functional
requirements: [F-LIFE-03]
module: lifecycle
run: manual
platform: macos
tags: [lifecycle]
external_id: TC-LIFE-003
---

# TC-003: Quit leaves no process and restarts normally

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed and running

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Quit Echo"; check Activity Monitor for `echoctl` | No process left |
| 2 | Open Echo from Applications | It starts without the first-run dialogs |

## Expected Final State
Quit is clean.
