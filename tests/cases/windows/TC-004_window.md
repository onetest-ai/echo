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
- Echo running with calls, not signed in to Microsoft 365
- A fresh install, or none run from this `echoctl.exe` path before: Windows caches a program's name per path (MuiCache), so a path an older build ran from may keep "echoctl.exe"

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Tray → "Open Echo…" | The window (WebView2) shows Today, calls, tabs and the calendar as on macOS |
| 2 | Settings → General: read the "Start at login" row | "Echo runs in the notification area from when you sign in." — not "menu bar" (#420) |
| 3 | Open the calendar | Its note says to connect Microsoft 365 "from the octopus in the notification area (Calendar)" — not "menu bar" |
| 4 | Right-click Echo's taskbar button | Its menu names the app Echo, not echoctl.exe |

## Expected Final State
The window works on Windows and speaks Windows' words.
