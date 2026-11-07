---
id: TC-009
title: The status bar's Actions button opens the palette and Escape closes it
priority: low
type: functional
requirements: [F-WS-03, F-WS-07]
module: shell
run: agent
platform: any
tags: [shell, palette, status-bar]
---

# TC-009: The status bar's Actions button opens the palette and Escape closes it

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and click "Actions" in the status bar (it reads "Actions ⌘K" or "Actions Ctrl+K") | The dialog "Actions" opens |
| 2 | Press Escape | The dialog closes |
| 3 | Press `ControlOrMeta+k`, then `ControlOrMeta+k` again | The dialog opens, then closes (the shortcut toggles it) |

## Expected Final State
The palette opens from both entries and closes.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
