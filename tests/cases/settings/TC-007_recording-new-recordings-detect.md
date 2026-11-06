---
id: TC-007
title: Recording: New recordings Detect changes the Record button's language
priority: medium
type: functional
requirements: [F-SET-07]
module: settings
run: agent
platform: any
tags: [settings, languages]
---

# TC-007: Recording: New recordings Detect changes the Record button's language

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** the browser's local storage key `echo.recordLanguage`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/recording`; read the "New recordings" row | "English" is pressed, "Detect" is not; the top bar select "Language of the call" shows "English" |
| 2 | Click "Detect" in the "New recordings" row | "Detect" is pressed; the select "Language of the call" shows "Detect the language" |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"languages"' "$ECHOCTL_HOME/.echo/config.json" -A3` | Still only `"en"`: the choice is not in `config.json` |
| 4 | Reload `{{base_url}}/#settings/recording` | "Detect" is still pressed (this browser keeps the choice) |

## Expected Final State
New recordings is on Detect.

## Teardown
- Click "English" in the "New recordings" row.

## Notes
- Do not press Record: nothing is recorded on test data.
