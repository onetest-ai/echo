---
id: TC-004
title: The window on Windows
priority: high
type: functional
module: windows
run: manual
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

## Expected Final State
The window works on Windows.
