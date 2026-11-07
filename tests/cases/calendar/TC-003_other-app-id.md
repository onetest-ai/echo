---
id: TC-003
title: Another app (client) ID is used for sign-in, and default returns
priority: low
type: functional
requirements: [F-M365-01, F-SET-13]
module: calendar
run: person
platform: macos
tags: [calendar, m365, signin]
external_id: TC-CAL-003
---

# TC-003: Another app (client) ID is used for sign-in, and default returns

**Module:** Calendar | **Priority:** Low | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar → Calendar → "Change app (client) ID…", enter your organisation's ID | A new sign-in is required |
| 2 | Connect Microsoft 365 | The consent page shows that app |
| 3 | Change the ID to `default` and connect again | The default app is used |

## Expected Final State
The sign-in uses the chosen app ID, and `default` restores Echo's own.
