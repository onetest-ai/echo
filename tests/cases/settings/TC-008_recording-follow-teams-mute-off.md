---
id: TC-008
title: Recording: Follow Teams' mute off is saved to config.json
priority: medium
type: functional
requirements: [F-SET-08, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, recording]
---

# TC-008: Recording: Follow Teams' mute off is saved to config.json

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `follow_app_mute`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/recording`; read the switch "Follow Teams' mute" | It is on (`aria-checked` true) |
| 2 | Click the switch "Follow Teams' mute" | It is off (`aria-checked` false) |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"follow_app_mute"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"follow_app_mute": false,` |
| 4 | Reload the page | The switch is still off |

## Expected Final State
Follow Teams' mute is off in the page and in `config.json`.

## Teardown
- Click the switch "Follow Teams' mute" so it is on again.
