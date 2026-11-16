---
id: TC-010
title: A meeting's card opens beside it and closes
priority: medium
type: functional
requirements: [F-CAL-07, F-CAL-08]
module: window
run: agent
platform: any
tags: [calendar]
external_id: TC-WIN-009
auto: [tests/auto/specs/S-02.spec.mjs]
---

# TC-010: A meeting's card opens beside it and closes

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Built after about 21:50 the meetings to come start sooner (#364: at 22:50 Globex design review starts 8 minutes after the build, tests/fixtures/README.md): run the steps that need it still to come at once after the build, or build earlier

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Globex design review |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calendar/week` | Today's column has Globex design review |
| 2 | Click "Globex design review" | A card opens beside the meeting, fully inside the calendar's grid (never over the activity bar or Day / Week): title with "Close" (×) beside it, "Today · HH:MM–HH:MM · in N min" ("in 1 h" right after the data is built), the chips (kind, "+ Thread"), "Record when it starts" and "Prepare" on one line, and PEOPLE with "Sam Rivera" (organizer) |
| 3 | Scroll the grid a little | The card stays with the meeting and fully inside the grid (level with it where there is room; near the grid's bottom it is held inside) |
| 4 | Click "Close" on the card | The card goes |
| 5 | `#calendar/day`; click a meeting that spans the whole day column (on standard data "Umbrella pricing review" in the evening) | Its card is inside the grid over the block's right part, level with it — not at the window's left over the activity bar (#338) |

## Expected Final State
The card opens beside its meeting, stays with it inside the calendar's grid on scroll, and closes.
