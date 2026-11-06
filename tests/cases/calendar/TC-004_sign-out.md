---
id: TC-004
title: Sign out removes the token and falls back to today's agenda
priority: medium
type: functional
module: calendar
run: manual
platform: macos
tags: [calendar, m365]
external_id: TC-CAL-002
---

# TC-004: Sign out removes the token and falls back to today's agenda

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)
- Echo signed in

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar → Calendar → "Sign out" | The menu offers "Connect Microsoft 365…" again |
| 2 | Check `~/Library/Application Support/echo/graph.json` | It is gone |
| 3 | Open the window → Calendar | Today's agenda only, with the notice to connect; no error |

## Expected Final State
Signed out cleanly: no token left, the calendar shows today's agenda only.
