---
id: TC-018
title: AI & minutes: Live notes during calls on shows "Live notes on" in the status bar
priority: medium
type: functional
requirements: [F-SET-12, F-WS-07]
module: settings
run: agent
platform: any
tags: [settings, live-notes]
---

# TC-018: AI & minutes: Live notes during calls on shows "Live notes on" in the status bar

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config dir>/ai.json` key `live_notes`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; read the switch "Live notes" and the status bar | The switch is off; the status bar does not say "Live notes on" |
| 2 | Click the switch "Live notes" | It is on; the status bar now shows "Live notes on" |
| 3 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/ai.json"` | The file has `"live_notes": true` |

## Expected Final State
Live notes are on in the page, the status bar and `ai.json`.

## Teardown
- Click the switch "Live notes" so it is off (`echoctl ai live off` does the same).
