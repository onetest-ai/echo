---
id: TC-021
title: About me: the name is saved to config.json
priority: high
type: functional
requirements: [F-SET-14, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, me]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-021: About me: the name is saved to config.json

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `me`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/me`; read the field "Your name" | It holds `Sam Rivera`; "Your email" holds `sam@initech.example` |
| 2 | Clear the field "Your name", type `Sam Rivera Jr` and press Tab | The field holds `Sam Rivera Jr` |
| 3 | Wait 1 s, run `. <dir>/env && grep -n -A3 '"me"' "$ECHOCTL_HOME/.echo/config.json"` | The `me` object has `"name": "Sam Rivera Jr"` and `"email": "sam@initech.example"` |
| 4 | Reload the page | "Your name" still holds `Sam Rivera Jr` |

## Expected Final State
The user's name is saved, the email untouched.

## Teardown
- Set "Your name" back to `Sam Rivera` and press Tab.
