---
id: TC-006
title: Recording: a language is added and removed in the calls' languages
priority: high
type: functional
requirements: [F-SET-07, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, languages]
---

# TC-006: Recording: a language is added and removed in the calls' languages

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `languages`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/recording`; read "Languages a call may be in" | One chip "English" with a × button; the row says each call is in one of these, detected from its first sentences |
| 2 | Choose "Ukrainian" in the "Add a language" list | Two chips: "English" and "Ukrainian"; "Ukrainian" is no longer in the "Add a language" list |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"languages"' "$ECHOCTL_HOME/.echo/config.json" -A3` | The list holds `"en"` and `"uk"` |
| 4 | Click × on the "Ukrainian" chip (button "Remove Ukrainian") | One chip "English" remains |
| 5 | Wait 1 s, run `. <dir>/env && grep -n '"languages"' "$ECHOCTL_HOME/.echo/config.json" -A3` | The list holds `"en"` only |

## Expected Final State
Only English is listed, as at the start.

## Teardown
- If a "Ukrainian" chip remains, remove it with its ×.
