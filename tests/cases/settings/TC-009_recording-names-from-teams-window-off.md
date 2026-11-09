---
id: TC-009
title: Recording: Read the Teams window off is saved to config.json, and the row says what it costs
priority: medium
type: functional
requirements: [F-SET-08, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, recording]
---

# TC-009: Recording: Read the Teams window off is saved to config.json, and the row says what it costs

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `capture_ui`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/recording`; read the row "Read the Teams window" | Its switch is on; its text names who is speaking, mute sync and the call's name, says Teams answers every read itself ("about 10–20% of its CPU") and that calls are still recorded and transcribed; the row "Shared screens" says it reads Teams' window too, apart from this switch |
| 2 | Click it | It is off |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"capture_ui"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"capture_ui": false,` |

## Expected Final State
Read the Teams window is off in the page and in `config.json`.

## Teardown
- Click the switch so it is on again.

## Notes
- "Shared screens" is not pressed on purpose: switching it on asks macOS for Screen Recording.
- What the switch turns off during a call (names, mute sync, the same-call check by Teams' timer, the call's name) is in the Rust test `config::tests::the_call_window_is_read_only_when_allowed_and_never_for_a_call_fed_from_files`; a call fed from files never reads the real Teams window, whatever the switch says.
