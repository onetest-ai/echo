---
id: TC-005
title: Recording: Other Teams calls set to Manual is saved to config.json
priority: high
type: functional
requirements: [F-SET-06, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, recording]
---

# TC-005: Recording: Other Teams calls set to Manual is saved to config.json

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `unscheduled`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/recording`; read the "Other Teams calls" row | "Ask" is pressed, "Manual" is not |
| 2 | Click "Manual" in the "Other Teams calls" row | "Manual" is pressed; the "Calendar meetings" row is unchanged |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"unscheduled"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"unscheduled": "manual",` |

## Expected Final State
Other Teams calls is Manual, in the page and in `config.json`.

## Teardown
- Click "Ask" in "Other Teams calls".
