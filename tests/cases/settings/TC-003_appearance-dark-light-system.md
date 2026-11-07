---
id: TC-003
title: Appearance Dark, Light and System switch the window
priority: medium
type: functional
requirements: [F-SET-04, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, appearance]
---

# TC-003: Appearance Dark, Light and System switch the window

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** the browser's local storage key `echo.theme` (per computer, not in the data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/general`; in the "Appearance" row read the three buttons | "System" is pressed; the page element `html` has no `data-theme` attribute |
| 2 | Click "Dark" in the Appearance row | "Dark" is pressed; `document.documentElement.dataset.theme` is `dark` |
| 3 | Click "Light" | "Light" is pressed; `document.documentElement.dataset.theme` is `light` |
| 4 | Click "Dark", then reload `{{base_url}}/#settings/general` | After the reload "Dark" is still pressed and `data-theme` is `dark` (the choice is kept by this browser) |
| 5 | Click "System" | "System" is pressed; `html` has no `data-theme` attribute |

## Expected Final State
Appearance is back on System.

## Teardown
- Click "System" in General -> Appearance (it removes the browser's `echo.theme` value).

## Notes
- Only the Appearance row of General is read; the other General rows show the real machine and are never asserted.
