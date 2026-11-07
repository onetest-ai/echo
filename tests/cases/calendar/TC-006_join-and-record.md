---
id: TC-006
title: Join and record opens Teams and records as soon as you are in
priority: high
type: functional
requirements: [F-CAL-14]
module: calendar
run: person
platform: macos
tags: [calendar, recording]
external_id: TC-CAL-005
---

# TC-006: Join and record opens Teams and records as soon as you are in

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)
- A Teams meeting starting within minutes

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Click the meeting → "Join and record" (window or menu → Upcoming) | Teams opens the meeting; Echo is armed |
| 2 | Get admitted | Recording starts at once; the call's title and people are the event's |

## Expected Final State
One click joins the meeting and records it.
