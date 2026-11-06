---
id: TC-002
title: Install and first run on Windows
priority: critical
type: functional
requirements: [F-WIN-01]
module: windows
run: manual
platform: windows
tags: [windows, install]
external_id: TC-WINOS-002
---

# TC-002: Install and first run on Windows

**Module:** Windows | **Priority:** Critical | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Unzip to `%LOCALAPPDATA%\Programs\echo`; run `echoctl.exe app autostart on`; double-click `echoctl.exe` | At most a SmartScreen warning (More info → Run anyway); no console window stays |
| 2 | Go through the folder and model dialogs | As on macOS (install TC-002, TC-004); the tray icon appears |
| 3 | Look in `%APPDATA%\echo` | The config is there |

## Expected Final State
Echo installs per user and starts in the tray.
