---
id: TC-002
title: Connect Microsoft 365 signs in and fills the calendar
priority: high
type: functional
requirements: [F-M365-01, F-PRIV-06]
module: calendar
run: manual
platform: macos
tags: [calendar, m365, signin]
external_id: TC-CAL-001
---

# TC-002: Connect Microsoft 365 signs in and fills the calendar

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)
- Echo running, not signed in

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar → Calendar → "Connect Microsoft 365…" | The system browser opens Microsoft's sign-in |
| 2 | Sign in | The tab says "Signed in to Echo. You can close this tab."; a "Calendar connected" notification names the account |
| 3 | Open the window → Calendar → Week | The week's meetings are there |
| 4 | Run `ls -l ~/Library/Application\ Support/echo/graph.json` | `-rw-------`: readable only by you |

## Expected Final State
Echo is signed in to Microsoft 365, the calendar shows the week, and the token file is private.
