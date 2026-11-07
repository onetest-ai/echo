---
id: TC-014
title: AI & minutes: Claude in the window without permission prompts shows its warning and is off
priority: high
type: functional
requirements: [F-SET-12]
module: settings
run: agent
platform: any
tags: [settings, claude]
---

# TC-014: AI & minutes: Claude in the window without permission prompts shows its warning and is off

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; find the card "Claude in the window" | A row "Claude in the window: no permission prompts" shows |
| 2 | Read the row's text | It names `--dangerously-skip-permissions` and says Claude "runs any command and tool without asking" and "Takes effect the next time the session starts." |
| 3 | Read the row's switch | It is off; the row does not show the text "On." |
| 4 | Run `. <dir>/env && grep -n '"folder_skip_permissions"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"folder_skip_permissions": false,` |

## Expected Final State
The dangerous switch is off and its warning is on the page.

## Notes
- The switch is never pressed: it changes how Claude Code starts for every later session.
