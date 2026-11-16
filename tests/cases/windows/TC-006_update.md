---
id: TC-006
title: Update on Windows
priority: low
type: functional
requirements: [F-WIN-05]
module: windows
run: person
platform: windows
tags: [windows, update]
external_id: TC-WINOS-006
---

# TC-006: Update on Windows

**Module:** Windows | **Priority:** Low | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)
- A newer release

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Tray → Update | The MSI is downloaded, its SHA-256 checked, installed silently (no admin prompt); Echo restarts on the new version (`%TEMP%\echo-update-<version>\install.log`) |

## Expected Final State
Windows updates install themselves safely.
