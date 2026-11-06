---
id: TC-010
title: A meeting's card opens beside it and closes
priority: medium
type: functional
module: window
run: agent
platform: any
tags: [calendar]
external_id: TC-WIN-009
---

# TC-010: A meeting's card opens beside it and closes

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Globex design review |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calendar/week` | Today's column has Globex design review |
| 2 | Click "Globex design review" | A card opens beside the meeting, fully inside the window: title, date and time, Organizer "Sam Rivera", People, "Record when it starts", "+ Thread", "Prepare" and "Close" |
| 3 | Scroll the grid a little | The card stays level with the meeting |
| 4 | Click "Close" on the card | The card goes |

## Expected Final State
The card opens beside its meeting, follows it on scroll and closes.
