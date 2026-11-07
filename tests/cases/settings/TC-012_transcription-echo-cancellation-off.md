---
id: TC-012
title: Transcription: Echo cancellation off is saved to config.json
priority: medium
type: functional
requirements: [F-SET-11, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, audio]
---

# TC-012: Transcription: Echo cancellation off is saved to config.json

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `echo_cancel`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/transcription`; read the switch "Echo cancellation" | It is on |
| 2 | Click it | It is off |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"echo_cancel"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"echo_cancel": false,` |

## Expected Final State
Echo cancellation is off in the page and in `config.json`.

## Teardown
- Click the switch so it is on again.
