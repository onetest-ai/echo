---
id: TC-021
title: A card's thread chips say why each is there; + Thread adds one by hand and × takes only a hand-added one out
priority: high
type: functional
requirements: [F-CAL-12]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, card, threads]
---

# TC-021: A card's thread chips say why each is there; + Thread adds one by hand and × takes only a hand-added one out

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today

## Test Data

| Meeting (id) | Chips | Why (chip title) | × |
|---|---|---|---|
| Steering prep (`ev-pinned`) | Acme rollout | added by hand | yes |
| Globex weekly (`ev-globex-weekly`) | Globex pilot | the client's people | no |
| Acme renewal follow-up (`ev-renewal-call`) | Acme renewal | its subject | no |
| Acme weekly sync (`ev-acme-sync`, recorded) | Acme rollout | its meeting series | no, and no + Thread |
| Acme design review (`ev-m01`) | none |  | + Thread offered |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day", click the block `[data-ev="ev-pinned"]` (Steering prep) | The chip "Acme rollout" (title "added by hand") with a × (`aria-label="Take it out of Acme rollout"`) and the button "+ Thread" |
| 2 | Click the block `[data-ev="ev-globex-weekly"]` (Globex weekly, to come 37 minutes after the build), then `[data-ev="ev-renewal-call"]` (Acme renewal follow-up) | Chips "Globex pilot" (title "the client's people") and "Acme renewal" (title "its subject"), neither with a × |
| 3 | Click the block `[data-ev="ev-acme-sync"]` (Acme weekly sync) | The chip "Acme rollout" (title "its meeting series") with no × and no "+ Thread" button |
| 4 | Click the block `[data-ev="ev-m01"]` (Acme design review), then "+ Thread" | A menu lists the open threads (Acme rollout, Acme renewal, Globex pilot…), the thread picker "Every occurrence →", and "New thread from this meeting…" (do not choose it) |
| 5 | Choose "Globex pilot" | The chip "Globex pilot" appears with a × |
| 6 | Click the × of the chip "Globex pilot" | The chip is gone; the button "+ Thread" remains |

## Expected Final State
Acme design review is in no thread again.

## Teardown
- If the chip "Globex pilot" is still on Acme design review, click its ×

## Notes
- Pins and unpins the event (no Claude Code).
