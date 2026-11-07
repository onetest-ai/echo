---
id: TC-008
title: Notifications and the tray icon on Windows
priority: high
type: functional
requirements: [F-WIN-06]
module: windows
run: person
platform: windows
tags: [windows, notifications]
external_id: TC-WINOS-008
---

# TC-008: Notifications and the tray icon on Windows

**Module:** Windows | **Priority:** High | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl notify "Test" "Hello"`; finish a recording and click the toast | Toasts say Echo with the octopus; clicking opens the window |
| 2 | Switch Windows between light and dark | The idle tray icon is black on light, white on dark, within seconds |

## Expected Final State
Echo looks right in Windows' notifications and tray.
