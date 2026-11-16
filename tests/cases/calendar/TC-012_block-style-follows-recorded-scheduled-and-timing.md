---
id: TC-012
title: A block is green when recorded, tagged Will record when scheduled, and dimmed when past
priority: high
type: functional
requirements: [F-CAL-04, F-CAL-03]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, blocks]
---

# TC-012: A block is green when recorded, tagged Will record when scheduled, and dimmed when past

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today
- Built less than 15 minutes ago: meetings "live" at the build (started 10 minutes before it) end 20 minutes after it and then read as past; "to come" ones start 37 minutes after it

## Test Data

| Meeting (id) | Timing | Recorded | Scheduled | Classes on the block | Tag in the block |
|---|---|---|---|---|---|
| Acme weekly sync (`ev-acme-sync`) | past | call | no | `recorded` `past` | none |
| Globex steering (`ev-globex-steering`) | past | imported | no | `recorded` `past` | none |
| Acme design review (`ev-m01`, 22:28-type) | past | no | no | `past` | none |
| Globex office hours (`ev-m02`) | live | no | yes (Ukrainian) | `scheduled` | "Will record" |
| Initech architecture board (`ev-m04`) | to come | no | yes | `scheduled` | "Will record" |
| Vendor demo (`ev-m07`) | live | no | no | neither `past`, `recorded` nor `scheduled` | none |
| Initech team lunch (`ev-m08`) | to come | no | no | neither | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day" | The title is today's weekday and date; the day column is marked today and has the line "now" (`.now`) |
| 2 | Read the classes and the small text of `ev-acme-sync` and `ev-globex-steering` | Both have `ev`, `online`, `recorded`, `past`; neither has a tag |
| 3 | Read the block `ev-m01` (Acme design review) | `ev`, `online`, `past` (and `narrow` if it sits beside three or more); no `recorded`, no tag |
| 4 | Read the blocks `ev-m02` and `ev-m04` | Both have `scheduled` and a tag "Will record" (`.tag.will`); not `past`; not `recorded` |
| 5 | Read the blocks `ev-m07` and `ev-m08` | Only `ev`, `online` and `narrow` (each sits in a group of more than four lanes): no `past`, `recorded`, `scheduled`, `transcript`, `declined`, `cancelled` and no tag |

## Expected Final State
Each block shows the style of its state.

## Notes
- Creates or changes nothing. Without Microsoft 365 the agenda never says a Teams transcript exists, so the `transcript` class and the "Transcript" tag, and the "No notetakers" tag, cannot appear on this data (`meeting.transcript` known and `meeting.flags` need Microsoft 365).
