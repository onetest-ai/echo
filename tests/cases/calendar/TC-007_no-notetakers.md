---
id: TC-007
title: A meeting marked no_notetakers is labelled so
priority: low
type: functional
requirements: [F-CAL-05]
module: calendar
run: person
platform: macos
tags: [calendar]
external_id: TC-CAL-006
---

# TC-007: A meeting marked no_notetakers is labelled so

**Module:** Calendar | **Priority:** Low | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)
- A meeting with `no_notetakers` in its title or description

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open it in the Calendar | It is labelled "No notetakers" |

## Expected Final State
The organiser's opt-out is visible on the meeting.
