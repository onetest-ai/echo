---
id: TC-014
title: The status bar shows Actions and Claude, and no AI profile (the top bar's chip shows it)
priority: low
type: functional
requirements: [F-WS-07]
module: shell
run: agent
platform: any
tags: [shell, status-bar]
---

# TC-014: The status bar shows Actions and Claude, and no AI profile (the top bar's chip shows it)

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and wait for the text "Your day" | A status bar runs along the foot of the window |
| 2 | Read the status bar | A button "Actions ⌘K" ("Actions Ctrl+K" off macOS) and a button "Claude"; no "AI:" (the default profile is the top bar's account chip) |
| 3 | Look for "Live notes on" and for "Minutes for" | Neither shows (live notes are off; no minutes are due soon) |

## Expected Final State
The status bar shows its two buttons.

## Notes
- "Claude" is never pressed: it starts a terminal session. The status bar changing with live notes, and the account chip with the default profile, are checked in settings/ cases.
