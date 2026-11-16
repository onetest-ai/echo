---
id: TC-004
title: Recording: Calendar meetings set to Manual is saved to config.json
priority: high
type: functional
requirements: [F-SET-06, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, recording]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-004: Recording: Calendar meetings set to Manual is saved to config.json

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `scheduled`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/recording`; read the "Calendar meetings" row | "Ask" is pressed, "Manual" is not |
| 2 | In Bash run `. <dir>/env && grep -n '"scheduled"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"scheduled": null,` |
| 3 | Click "Manual" in the "Calendar meetings" row | "Manual" is pressed, "Ask" is not; the "Other Teams calls" row is unchanged ("Ask" pressed) |
| 4 | Wait 1 s, run `. <dir>/env && grep -n '"scheduled"' "$ECHOCTL_HOME/.echo/config.json"` again | Prints `"scheduled": "manual",` |
| 5 | Reload `{{base_url}}/#settings/recording` | "Manual" is still pressed in "Calendar meetings" |

## Expected Final State
Calendar meetings is Manual, in the page and in `config.json`.

## Teardown
- Click "Ask" in "Calendar meetings"; `config.json` then holds `"scheduled": "ask"` (same behaviour as the unset default).
