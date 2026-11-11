---
id: TC-011
title: The native window opens and closes with its Dock icon
priority: medium
type: functional
requirements: [F-LIFE-08]
module: window
run: agent
platform: macos
tags: [window, native]
external_id: TC-WIN-001
---

# TC-011: The native window opens and closes with its Dock icon

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo running in the menu bar with at least one recorded call: the dev build on test data (`make_data.py <dir> --ai fake`, `. <dir>/env`, `echoctl app run --background` without `--no-tray`), or an installed Echo
- **No other Echo runs:** `pgrep -fl echoctl` lists only this run's app, else BLOCKED (as `recording/TC-019`); Hide from the Dock off (`hide_dock_icon` false)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar octopus → "Open Echo…" | The window opens in front; Echo has a Dock icon |
| 2 | Close the window with its red button | The window closes; Echo stays in the menu bar and the Dock; a click on its Dock icon opens the window again (F-LIFE-08) |
| 3 | Open a recent call from the menu | The window opens with that call selected |

## Expected Final State
The window opens and closes without quitting Echo, and a recent call opens selected.

## Notes
- Agent-run where no other Echo runs (run 2026-11-11: 3/3), driven by the app's pid through System Events as `recording/TC-019` describes (the window list, its `AXCloseButton`, the Dock item). Step 2 used to say the Dock icon goes; F-LIFE-08 says it stays unless Hide from the Dock, and Echo does that.
