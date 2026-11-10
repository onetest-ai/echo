---
id: TC-003
title: The sidebar flips with ⌘\ (Ctrl+\)
priority: low
type: functional
requirements: [F-WS-01]
module: shell
run: agent
platform: any
tags: [shell, activity-bar, keyboard]
auto: [tests/auto/specs/S-13.spec.mjs]
---

# TC-003: The sidebar flips with ⌘\ (Ctrl+\)

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and press `ControlOrMeta+\` | The button reads "Fold the sidebar" |
| 2 | Press `ControlOrMeta+\` again | The button reads "Expand the sidebar" |

## Expected Final State
The sidebar is folded, as it began.

## Teardown
- If the button reads "Fold the sidebar", press the shortcut again.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
