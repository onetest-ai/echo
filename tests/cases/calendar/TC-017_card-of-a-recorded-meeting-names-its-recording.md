---
id: TC-017
title: The card of a recorded meeting names its recording: the call's state and length, or from the Teams transcript
priority: medium
type: functional
requirements: [F-CAL-07]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, card]
---

# TC-017: The card of a recorded meeting names its recording: the call's state and length, or from the Teams transcript

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today

## Test Data

| Meeting (id) | Line | Value |
|---|---|---|
| Acme weekly sync (`ev-acme-sync`) | Recording | Minutes, 30 min |
| Initech planning (`ev-initech-plan`) | Recording | Transcript, 25 min |
| Umbrella intro call (`ev-umbrella-intro`) | Recording | Failed, 20 min |
| Globex steering (`ev-globex-steering`) | Call | from the Teams transcript, 45 min |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day", click the block `[data-ev="ev-acme-sync"]` (Acme weekly sync) | The card shows the title, "HH:MM to HH:MM", a kind chip "Client" (title "From its invite — change it for every meeting of the series"), Organizer "Zoe Martin", People 6, and Recording "Minutes, 30 min" |
| 2 | Click the block `[data-ev="ev-initech-plan"]` (Initech planning) | Recording "Transcript, 25 min" |
| 3 | Click the block `[data-ev="ev-umbrella-intro"]` (Umbrella intro call) | Recording "Failed, 20 min" |
| 4 | Click the block `[data-ev="ev-globex-steering"]` (Globex steering) | The label is "Call" and the value "from the Teams transcript, 45 min" |
| 5 | Click "Close" | The card is gone |

## Expected Final State
No card is open.

## Notes
- Creates or changes nothing.
