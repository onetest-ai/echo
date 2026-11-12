---
id: TC-007
title: ⌘K on Today lists Today's actions and the user's tasks
priority: low
type: functional
requirements: [F-WS-03]
module: shell
run: agent
platform: any
tags: [shell, palette]
auto: [tests/auto/specs/S-13.spec.mjs]
---

# TC-007: ⌘K on Today lists Today's actions and the user's tasks

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and wait for the text "Your day"; press `ControlOrMeta+k` | A dialog "Actions" opens with the chip "On: Today ×" |
| 2 | Read the group MY WORK | An item "Close: Send Acme the SSO checklist" and an item "Claude: Send Acme the SSO checklist" |
| 3 | Read the group ECHO | "Record the next call", "Record a memo", "Vocabulary", "Show or hide Inbox", "Expand or fold the sidebar", "Check for updates" |
| 4 | Press Escape | The dialog closes |

## Expected Final State
The palette listed Today's actions; nothing was run.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
- "Close:" changes the task and "Claude:" starts Claude Code: neither is run.
