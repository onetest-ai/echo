---
id: TC-008
title: Declined and cancelled meetings are greyed and not offered for recording
priority: low
type: functional
requirements: [F-CAL-05]
module: calendar
run: manual
platform: macos
tags: [calendar]
external_id: TC-CAL-007
---

# TC-008: Declined and cancelled meetings are greyed and not offered for recording

**Module:** Calendar | **Priority:** Low | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)
- A declined meeting and a cancelled one today

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the Calendar | Both are greyed; the cancelled one is struck through |
| 2 | Open each | Neither offers "Record when it starts" |
| 3 | Open Today | Neither is in Your day |

## Expected Final State
Meetings you won't attend are shown as such and never offered for recording.
