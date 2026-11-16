---
id: TC-023
title: Day and Week show the same meeting; the card stays open when the view is switched
priority: low
type: functional
requirements: [F-CAL-01, F-CAL-07]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, day, week]
---

# TC-023: Day and Week show the same meeting; the card stays open when the view is switched

**Module:** Calendar | **Priority:** Low | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today

## Test Data

| Field | Value |
|---|---|
| Meeting | Acme weekly sync (`ev-acme-sync`) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day" | The title is today's date with the weekday; one column; "Day" is pressed |
| 2 | Click the block `[data-ev="ev-acme-sync"]` (Acme weekly sync) | Its card opens |
| 3 | Click "Week" | The URL becomes `#calendar/week`, seven columns, the title is a range; the card of Acme weekly sync is still open |
| 4 | Click "Day" | One column again; the card is still open |
| 5 | Click "Close" | The card is gone |

## Expected Final State
Day view, no card.

## Notes
- Creates or changes nothing.
