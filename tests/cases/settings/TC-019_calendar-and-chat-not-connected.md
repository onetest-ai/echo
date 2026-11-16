---
id: TC-019
title: Calendar & chat says Not connected and offers Sign in…
priority: high
type: functional
requirements: [F-SET-13]
module: settings
run: agent
platform: any
tags: [settings, microsoft-365]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-019: Calendar & chat says Not connected and offers Sign in…

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/calendar` | The heading "Calendar & chat" and the card "Microsoft 365" show |
| 2 | Read the row "Account" | It says "Not connected: the calendar shows only what your tray knows about today." and has a button "Sign in…"; there is no "Sync now" and no "Sign out" |
| 3 | Read the row "App registration" | It says "The default (CodeMie). Your IT can register another." with a button "Change…" |
| 4 | Read the row "Meeting chat" | It has a switch "Meeting chat" |

## Expected Final State
The section shows the signed-out state.

## Notes
- "Sign in…" and "Change…" are not pressed: a sign-in needs a real Microsoft 365 account.
