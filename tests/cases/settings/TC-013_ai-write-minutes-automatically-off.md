---
id: TC-013
title: AI & minutes: Write minutes automatically off is saved to config.json
priority: high
type: functional
requirements: [F-SET-12, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, minutes]
---

# TC-013: AI & minutes: Write minutes automatically off is saved to config.json

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `auto_minutes`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; read the switch "Write minutes automatically" | It is on |
| 2 | Click it | It is off |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"auto_minutes"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"auto_minutes": false,` |
| 4 | Reload the page | The switch is still off |

## Expected Final State
Write minutes automatically is off in the page and in `config.json`.

## Teardown
- Click the switch so it is on again (`"auto_minutes": true`).
