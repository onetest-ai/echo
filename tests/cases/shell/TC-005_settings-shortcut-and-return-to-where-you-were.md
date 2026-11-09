---
id: TC-005
title: ⌘, opens Settings and an activity reopens where it was left
priority: low
type: functional
requirements: [F-WS-02]
module: shell
run: agent
platform: any
tags: [shell, keyboard]
---

# TC-005: ⌘, opens Settings and an activity reopens where it was left

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click the row "Acme weekly sync" (`[data-call$="acme-weekly-sync"]`) | The URL hash is `#call/<folder>/minutes` with a folder ending `-acme-weekly-sync` |
| 2 | Press `ControlOrMeta+1` | The hash is `#today` |
| 3 | Press `ControlOrMeta+2` | The hash is again `#call/<folder>/minutes` (the call, not the bare list) |
| 4 | Press `ControlOrMeta+,` | The hash is `#settings/general`; the settings page shows the heading "General" (`h2`, as written, not upper-cased) with the line "Start at login and the command line are this computer’s, …" under it |
| 5 | Click "Recording" in the left list, press `ControlOrMeta+1`, then `ControlOrMeta+,` | The hash is `#settings/recording` |

## Expected Final State
Settings reopened on the section left last.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
