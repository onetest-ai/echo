---
id: TC-015
title: Accepted, organizer, no answer, tentative, not answered and declined meetings are all drawn and offered the same
priority: medium
type: functional
requirements: [F-CAL-08, F-CAL-04]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, response]
---

# TC-015: Accepted, organizer, no answer, tentative, not answered and declined meetings are all drawn and offered the same

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today
- Built less than 15 minutes ago: meetings "live" at the build (started 10 minutes before it) end 20 minutes after it and then read as past; "to come" ones start 37 minutes after it

## Test Data

| Meeting (id) | Response | Timing |
|---|---|---|
| Vendor demo (`ev-m07`) | accepted | live |
| Initech architecture board (`ev-m28`, live) | organizer | live |
| Vendor demo (`ev-m31`) | none | live |
| Acme budget check (`ev-m14`) | tentativelyAccepted | live |
| Acme training dry run (`ev-m33`) | notResponded | live |
| Initech town hall (`ev-m18`) | declined | live |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day" | The day is shown |
| 2 | Look for the six blocks `[data-ev="ev-m07"]`, `ev-m28`, `ev-m31`, `ev-m14`, `ev-m33`, `ev-m18` | All six are drawn, each in its own lane at the same time |
| 3 | Read the classes of the six blocks | All six have `ev online`; none has `declined`, `cancelled`, `past` or a tag |
| 4 | Click each block in turn and read its card | Every card has the buttons "Join and record" and "Record it"; none says "You declined this meeting." |
| 5 | Read the kind chip of the card of Initech town hall | A kind chip is shown (guessed), like the others |

## Expected Final State
The six cards are alike; the answer to the invite shows nowhere in the calendar.

## Teardown
- Click "Close" on the card

## Notes
- Documents the window as it is: without Microsoft 365 the calendar is built from the agenda file and `window/mod.rs` `calendar()` sets `declined: false` for every agenda meeting, so the response is not read here. Today and Coming up today do read it (`prep/TC-006`). If the calendar starts to grey declined meetings, update this case.
- Creates or changes nothing; no button is pressed.
