---
id: TC-005
title: Start at sign-in, quit and the CLI on Windows
priority: high
type: functional
requirements: [F-WIN-04]
module: windows
run: manual
platform: windows
tags: [windows, lifecycle]
external_id: TC-WINOS-005
---

# TC-005: Start at sign-in, quit and the CLI on Windows

**Module:** Windows | **Priority:** High | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Sign out and in | Echo starts |
| 2 | Quit from the tray | No `echoctl.exe` left |
| 3 | Settings → Install command-line tool; in a new terminal `echoctl status` | It answers |

## Expected Final State
Lifecycle and the CLI work on Windows.
