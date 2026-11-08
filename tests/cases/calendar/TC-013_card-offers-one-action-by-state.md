---
id: TC-013
title: A meeting's card offers one action by its state: Open notes, Import, Don't record, Join & record or Record
priority: high
type: functional
requirements: [F-CAL-08]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, card, action]
---

# TC-013: A meeting's card offers one action by its state: Open notes, Import, Don't record, Join & record or Record

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today
- Built less than 15 minutes ago: meetings "live" at the build (started 10 minutes before it) end 20 minutes after it and then read as past; "to come" ones start 37 minutes after it

## Test Data

| Meeting (id) | State | Action(s) on the card (a Prepare or Show prep may sit beside them) | Line under them (`.why`; a prep adds its sentence: …) |
|---|---|---|---|
| Acme weekly sync (`ev-acme-sync`) | recorded, of a series | Open notes; under "Every meeting of this series": Record (pressed), Ask, Don't record | Minutes, 30 min |
| Globex steering (`ev-globex-steering`) | recorded (imported) | Open notes | A call made of its Teams transcript, 45 min |
| Acme design review (`ev-m01`) | past, not recorded, Teams link | Import Teams transcript | Not recorded. If Teams transcribed it: Echo makes a call of its transcript and chat; minutes, threads and the Brain follow. |
| Globex office hours (`ev-m02`) | live, scheduled, no link | Don't record | Echo starts recording a minute before it begins, transcribed as Ukrainian. |
| Initech architecture board (`ev-m04`) | to come, scheduled | Don't record | Echo starts recording a minute before it begins. |
| Vendor demo (`ev-m07`) | live, Teams link | Join & record, Record | Joins it in Teams and records both sides. … |
| Initech team lunch (`ev-m08`) | to come, no link | Record when it starts | Echo starts recording a minute before it begins. … |
| Acme help desk sync (`ev-m29`) | to come, Teams link | Record when it starts | Echo starts recording a minute before it begins. … |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day", click the block `[data-ev="ev-acme-sync"]` (Acme weekly sync) | The card has the primary button "Open notes" and no other action for this meeting; below it the series block "Every meeting of this series" with "Record" (pressed: the series rule is always) and "Don't record" (the series rule itself is `calendar/TC-018`) |
| 2 | Click the block `[data-ev="ev-globex-steering"]` (Globex steering) | "Open notes" only |
| 3 | Click the block `[data-ev="ev-m01"]` (Acme design review) | The primary button "Import Teams transcript" and the line (`.why`) "Not recorded. If Teams transcribed it: Echo makes a call of its transcript and chat; minutes, threads and the Brain follow."; no Record button (do not press the import here: `TC-014`) |
| 4 | Click the block `[data-ev="ev-m02"]` (Globex office hours) | The button "Don't record" and a line (`.why`) starting "Echo starts recording a minute before it begins, transcribed as Ukrainian." (a "Prepare" beside the button adds its own sentence after it); no Record button |
| 5 | Click the block `[data-ev="ev-m04"]` (Initech architecture board) | The button "Don't record" and a line starting "Echo starts recording a minute before it begins" |
| 6 | Click the block `[data-ev="ev-m07"]` (Vendor demo) | Two buttons: "Join & record" (with its language) and "Record" (title "Record without joining — you're in it already"; do not press them); run `browser_evaluate` with `() => { const a = document.querySelector('.detail .ec-acts'); return a.scrollWidth <= a.clientWidth }`: `true` — the strip fits the card (its pills wrap to a second line rather than spill) |
| 7 | Click the block `[data-ev="ev-m08"]` (Initech team lunch) | One button "Record when it starts" |
| 8 | Click the block `[data-ev="ev-m29"]` (Acme help desk sync, to come 37 minutes after the build) | One button "Record when it starts" (no "Join & record": it has a link but is not live) |

## Expected Final State
Every card showed the one action of its state; no recording was started.

## Teardown
- Click "Close" on the card

## Notes
- An open card can sit over later blocks: close it (its "Close") before clicking the next block.
- Creates or changes nothing: no action button is pressed. Recording from the card is `TC-001`, `TC-005`, `TC-006`.
- The cards of a declined, cancelled or no-notetakers meeting (no action) and of a meeting with a known Teams transcript or an import error need Microsoft 365: not on this data.
