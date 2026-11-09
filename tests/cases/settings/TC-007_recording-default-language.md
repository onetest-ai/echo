---
id: TC-007
title: "Recording: Default language makes another language the first, and the Record button's"
priority: medium
type: functional
requirements: [F-SET-07]
module: settings
run: agent
platform: any
tags: [settings, languages]
---

# TC-007: Recording: Default language

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `config.json` `languages` (its order)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `. <dir>/env && target/debug/echoctl config --languages en,uk`; navigate to `{{base_url}}/#settings/recording`; read the "Languages you record in" and "Default language" rows | Two chips, "English" and "Ukrainian"; the text says Echo never guesses a call's language; "Default language" shows "English"; the top bar select "Language of the call" shows "EN" and offers English and Ukrainian only (no "Detect") |
| 2 | Choose "Ukrainian" in the "Default language" select | "Default language" shows "Ukrainian"; the chips read "Ukrainian", "English" |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"languages"' "$ECHOCTL_HOME/.echo/config.json" -A3` | `"uk"` first, then `"en"`: the default is kept for the calls Echo starts by itself too |
| 4 | Reload `{{base_url}}/`; read the top bar select "Language of the call" | It shows "UK" |

## Expected Final State
Ukrainian is the default language: first in `config.json`, and the Record button's.

## Teardown
- Run `. <dir>/env && target/debug/echoctl config --languages en`.

## Notes
- Do not press Record: nothing is recorded on test data.
- Until 2026-11-09 this row was "New recordings: English or Detect", kept in the browser; nothing detects a call's language now.
