---
id: TC-005
title: A meeting set to record arms itself and records when joined
priority: critical
type: functional
requirements: [F-CAL-10]
module: calendar
run: manual
platform: macos
tags: [calendar, recording]
external_id: TC-CAL-004
---

# TC-005: A meeting set to record arms itself and records when joined

**Module:** Calendar | **Priority:** Critical | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)
- A Teams meeting 5 minutes from now with one other attendee

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Window → Calendar → the meeting → "Record when it starts" | It shows as set to record |
| 2 | Wait for its start | A minute before, the menu-bar icon turns armed |
| 3 | Join it | Recording starts; title and attendees come from the event |
| 4 | Leave and rejoin after it was recorded | No second recording starts by itself |
| 5 | After the call, open the meeting in the calendar | It shows as recorded and opens its call |

## Expected Final State
A scheduled meeting records once, from the calendar's event.
