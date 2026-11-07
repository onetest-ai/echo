---
id: TC-020
title: A card's kind chip says where its type comes from, and choosing another type makes it the user's
priority: medium
type: functional
requirements: [F-CAL-11]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, card, kind]
---

# TC-020: A card's kind chip says where its type comes from, and choosing another type makes it the user's

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today

## Test Data

| Meeting (id) | Type | Source | Chip title |
|---|---|---|---|
| Initech architecture board (`ev-m04`) | Internal | guess (dashed) | Guessed from who is invited |
| Acme weekly sync (`ev-acme-sync`) | Client | invite | From its invite — change it for every meeting of the series |
| Globex office hours (`ev-m02`) | Partner | you | You set this — for every meeting of the series |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day", click the block `[data-ev="ev-m04"]` (Initech architecture board) | The chip "Internal ▾" has the class `guess` and the title "Guessed from who is invited" |
| 2 | Choose "Partner" in `select[aria-label="Kind of meeting"]` | The chip reads "Partner ▾", has no class `guess`, and its title is "You set this — for every meeting of the series" |
| 3 | Click the block `[data-ev="ev-acme-sync"]` (Acme weekly sync) | The chip "Client ▾" has no `guess` and the title "From its invite — change it for every meeting of the series" |
| 4 | Click the block `[data-ev="ev-m02"]` (Globex office hours) | The chip "Partner ▾" with the title "You set this — for every meeting of the series" |
| 5 | Reload the page, open the block `[data-ev="ev-m04"]` (Initech architecture board) again | The chip still reads "Partner ▾" and is not dashed |

## Expected Final State
Initech architecture board is a Partner meeting by the user's choice.

## Notes
- Writes the type of that meeting (`meeting-types-of.json`); a guess cannot be restored by the window, so the case leaves the data changed: scenarios and single-case runs build fresh data.
