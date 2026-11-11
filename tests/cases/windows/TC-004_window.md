---
id: TC-004
title: The window on Windows
priority: high
type: functional
requirements: [F-WIN-03]
module: windows
run: person
platform: windows
tags: [windows, window]
external_id: TC-WINOS-004
---

# TC-004: The window on Windows

**Module:** Windows | **Priority:** High | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)
- Echo running with calls

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Tray → "Open Echo…" | The window (WebView2) shows Today, calls, tabs and the calendar as on macOS |
| 2 | Read Settings → General's "Start at login" row and, signed out of Microsoft 365, the calendar's note; right-click Echo's taskbar button | Windows' words: "runs in the notification area from when you sign in", "the octopus in the notification area" — never "menu bar"; the taskbar's menu names the app Echo, not echoctl.exe (#420) |

## Expected Final State
The window works on Windows and speaks Windows' words.
