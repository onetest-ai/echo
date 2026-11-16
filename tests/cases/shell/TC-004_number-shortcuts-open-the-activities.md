---
id: TC-004
title: ⌘1 to ⌘5 open Today, Calls, Calendar, Threads and Tasks
priority: low
type: functional
requirements: [F-WS-02]
module: shell
run: agent
platform: any
tags: [shell, keyboard]
auto: [tests/auto/specs/S-13.spec.mjs]
---

# TC-004: ⌘1 to ⌘5 open Today, Calls, Calendar, Threads and Tasks

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`, press `ControlOrMeta+2` | The URL hash is `#calls`; the button Calls has `aria-current=page` |
| 2 | Press `ControlOrMeta+3` | The hash starts with `#calendar` |
| 3 | Press `ControlOrMeta+4` | The hash starts with `#project`; the heading "Threads" shows |
| 4 | Press `ControlOrMeta+5` | The hash is `#tasks` |
| 5 | Press `ControlOrMeta+1` | The hash is `#today`; the text "Your day" shows |

## Expected Final State
Each shortcut opened its activity, in the bar's order.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
- Focus must not be in a text box.
