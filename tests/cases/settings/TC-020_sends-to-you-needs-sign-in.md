---
id: TC-020
title: Sends to you shows Teams and Mail waiting for Microsoft 365
priority: medium
type: functional
requirements: [F-SET-15]
module: settings
run: agent
platform: any
tags: [settings, deliver]
---

# TC-020: Sends to you shows Teams and Mail waiting for Microsoft 365

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/deliver` | The heading "Sends to you" shows |
| 2 | Read the rows | "Teams": "Your chat with yourself. Sign in to Microsoft 365 first (Settings → Calendar & chat)."; "Mail": "Your own mailbox — or the addresses a plugin was given. Sign in to Microsoft 365 first (Settings → Calendar & chat)." |
| 3 | Look for "Send a test" | None shows (nothing to test before a sign-in) |

## Expected Final State
Both ways say they need the Microsoft 365 sign-in.

## Notes
- Per-plugin ways appear here once a plugin that adds one is installed; none on test data.
