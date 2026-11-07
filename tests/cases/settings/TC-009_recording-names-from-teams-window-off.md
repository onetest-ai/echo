---
id: TC-009
title: Recording: Names from the Teams window off is saved to config.json
priority: medium
type: functional
requirements: [F-SET-08, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, recording]
---

# TC-009: Recording: Names from the Teams window off is saved to config.json

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `capture_ui`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/recording`; read the switch "Names from the Teams window" | It is on |
| 2 | Click it | It is off |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"capture_ui"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"capture_ui": false,` |

## Expected Final State
Names from the Teams window is off in the page and in `config.json`.

## Teardown
- Click the switch so it is on again.

## Notes
- "Shared screens" is not pressed on purpose: switching it on asks macOS for Screen Recording.
