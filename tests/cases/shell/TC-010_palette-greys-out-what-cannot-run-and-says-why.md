---
id: TC-010
title: A palette item that cannot run is greyed with the reason
priority: low
type: functional
requirements: [F-WS-03]
module: shell
run: agent
platform: any
tags: [shell, palette]
---

# TC-010: A palette item that cannot run is greyed with the reason

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click the row "Initech planning" | The call opens (it has a transcript and no minutes) |
| 2 | Press `ControlOrMeta+k` | The dialog opens with the chip "On: Initech planning ×" |
| 3 | Read the item "Open minutes in browser" | It is greyed (`aria-disabled` true) and its hint reads "no minutes yet" |
| 4 | Move to it with ArrowDown and press Enter | The dialog stays open; the URL does not change |
| 5 | Press Backspace on the empty box | The chip "On: Initech planning ×" goes; the group THIS CALL is gone |
| 6 | Press Escape | The dialog closes |

## Expected Final State
The unavailable item named its reason and did nothing; the target was dropped with Backspace.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
