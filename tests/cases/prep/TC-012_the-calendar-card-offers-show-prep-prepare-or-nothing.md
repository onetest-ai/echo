---
id: TC-012
title: A meeting's card offers Show prep (and Prepare again), Prepare, or nothing, by its prep and timing
priority: medium
type: functional
requirements: [F-PREP-06, F-PREP-03]
module: prep
run: agent
platform: any
data: standard
tags: [prep, calendar, card]
---

# TC-012: A meeting's card offers Show prep (and Prepare again), Prepare, or nothing, by its prep and timing

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)
- A meeting's block in the Day view is `[data-ev="<id>"]`

## Test Data

| Meeting (id) | Prep | Timing | Card shows |
|---|---|---|---|
| Globex sponsor update (`ev-m20`) | ready | to come | Show prep + Prepare again |
| Umbrella security Q&A (`ev-m11`) | ready | past | Show prep only |
| Initech team lunch (`ev-m08`) | none | to come | Prepare + "Prepare writes last time, what is open, what is new and who is coming." |
| Acme design review (`ev-m01`) | none | past | no prep line |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day`, wait for "Day" and click `[data-ev="ev-m20"]` | The card has a button "Show prep" and a link-style button "Prepare again" |
| 2 | Click `[data-ev="ev-m11"]` | The card has "Show prep" and no "Prepare again" |
| 3 | Click `[data-ev="ev-m08"]` | The card has a button "Prepare" beside its main one (on one line) and its line ends "Prepare writes last time, what is open, what is new and who is coming."; no "Show prep" |
| 4 | Click `[data-ev="ev-m01"]` | The card has no prep line at all |
| 5 | Click `[data-ev="ev-m11"]`, then "Show prep" | The URL becomes `#upcoming/<prep id>`; the title is "Umbrella security Q&A"; the prep is shown with "In short"; the people line in the header reads six names and "and 6 more"; no "Write it again" (the meeting is over) |

## Expected Final State
The prep of Umbrella security Q&A is open.

## Notes
- Creates or changes nothing: Prepare and Prepare again start Claude Code and are not pressed (`TC-013`).
