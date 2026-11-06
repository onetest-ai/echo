---
id: TC-011
title: The native window opens and closes with its Dock icon
priority: medium
type: functional
requirements: [F-LIFE-08]
module: window
run: manual
platform: macos
tags: [window, native]
external_id: TC-WIN-001
---

# TC-011: The native window opens and closes with its Dock icon

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo installed and running in the menu bar, with at least one recorded call

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar octopus → "Open Echo…" | The window opens; a Dock icon appears |
| 2 | Close the window with its red button | The Dock icon goes; Echo stays in the menu bar |
| 3 | Open a recent call from the menu | The window opens with that call selected |

## Expected Final State
The window opens and closes without quitting Echo, and a recent call opens selected.
