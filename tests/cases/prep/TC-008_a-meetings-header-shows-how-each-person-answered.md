---
id: TC-008
title: A meeting's header shows how each person answered the invite, the most important in the thread first, who declined last; a hover tells more, "+N more" the rest
priority: medium
type: functional
requirements: [F-PREP-02]
module: prep
run: agent
platform: any
data: standard
tags: [prep, people]
---

# TC-008: A meeting's header shows how each person answered the invite, the most important in the thread first, who declined last; a hover tells more, "+N more" the rest

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`
- Globex weekly (`ev-globex-weekly`, prep id `08bd9e1778b4234b`) is still to come: run before 21:00 local
- The header's people are `.doc-head .at`, a name `.at-p` (its answer is the `aria-label` of its `.at-s` mark: Organiser, Accepted, Tentative, No answer yet, Declined), its hover card `.at-card`; "+N more" is `.at-more`, its list `.at-pop`. The answers count is `.at-tally` on the line with the date

## Test Data

| Person | Answer | In the thread Globex pilot |
|---|---|---|
| Kim Park | organiser | sponsor, influence high, interest high, 3 calls |
| Noor Aziz | tentative | not placed, 3 calls |
| Mia Long, Joel Brandt, Sara Lind | accepted | not in it, no call yet |
| Ines Duarte | tentative | not in it, no call yet |
| Paul Weber, Theo Grant | no answer | not in it, no call yet |
| Lena Vogel | declined | — |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync", click "Coming up today" and then the row "Globex weekly" | The header "Globex weekly"; the date line ends with "4 yes · 2 maybe · 2 no answer · 1 declined" |
| 2 | Read the names under the date line | In this order: "Kim Park" (bold, Organiser), "Noor Aziz" (Tentative), "Mia Long" (Accepted), "Joel Brandt" (Accepted), "Ines Duarte" (Tentative), "Paul Weber" (No answer yet), "Sara Lind" (Accepted), then "+2 more". Each of the five guests has the tag "new"; Kim Park and Noor Aziz have none |
| 3 | Hover "Kim Park" | A card: "Kim Park · Organiser", "sponsor", "Influence high · interest high · 3 calls together" |
| 4 | Hover "+2 more" | A list headed "2 more": "Theo Grant" (No answer yet, "new", "first call"), a line, then "Lena Vogel" struck through (Declined, "first call": anyone with no calls yet says so) |
| 5 | Click "Globex sponsor update" in Coming up today | Its prep reads in one column: In short, Expected from you, Last time, Open with these people, New since then, then "Written HH:MM · sonnet" and "Write it again"; no heading "Who's coming" anywhere |

## Expected Final State
Nothing changed: the header was only read.

## Teardown
- Click "Coming up today" to fold it

## Notes
- The answers come from the calendar (each attendee's response); who declined is kept apart and shown last. The order is the People map's influence, then interest, in the meeting's threads, then calls together.
