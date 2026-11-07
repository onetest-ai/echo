---
id: TC-015
title: AI & minutes with only the built-in profile: default, no profile picker
priority: high
type: functional
requirements: [F-AI-01, F-AI-02, F-AI-07]
module: settings
run: agent
platform: any
tags: [ai, profiles]
---

# TC-015: AI & minutes with only the built-in profile: default, no profile picker

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; read the card "Profiles" | One row "Claude Code (as configured)" with the tags "Default for new calls" and "Built in", and the text "minutes: sonnet" and "live notes: haiku" |
| 2 | Look for a "Make default" button | None shows |
| 3 | Read the status bar | It starts with "AI: Claude Code (as configured)" |
| 4 | Read the selects in the top bar | Only "Language of the call"; there is no "Model for live notes and minutes" |
| 5 | In Bash run `echoctl ai status` | JSON with one profile: id `system`, name "Claude Code (as configured)", `"default": true` |

## Expected Final State
One built-in default profile, and no picker.

## Notes
- Which model names show (sonnet, haiku) are the built-in defaults.
