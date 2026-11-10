---
id: TC-010
title: A recorded call whose meeting had a prep has a Prep tab before Minutes; other calls have none
priority: medium
type: functional
requirements: [F-PREP-07, F-CALLS-07]
module: prep
run: agent
platform: any
data: standard
tags: [prep, calls, tabs]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-010: A recorded call whose meeting had a prep has a Prep tab before Minutes; other calls have none

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Tabs |
|---|---|
| Umbrella intro call (`umbrella-intro-call`) | Prep, Minutes, Transcript |
| Globex retro (`globex-retro`) | Prep, Minutes, Transcript |
| Globex standup (`globex-standup`) | Prep, Minutes, Transcript |
| Acme go-live check (`acme-go-live-check`) | Prep, Minutes, Transcript |
| Acme weekly sync (Today) | Minutes, Transcript, Chat, Screens (no Prep) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Umbrella intro call | The tabs read Prep, Minutes, Transcript, in this order |
| 2 | Click the tab "Prep" | The URL ends `/prep`; the prep is shown |
| 3 | Click the rows Globex retro, Globex standup and Acme go-live check in turn | Each shows Prep first, then Minutes, Transcript |
| 4 | Click the row Acme weekly sync | The tabs read Minutes, Transcript, Chat, Screens; no Prep; the page is on Minutes (the open tab "Prep" does not exist for it) and the URL ends with `/minutes`, not `/prep` (#148) |

## Expected Final State
Acme weekly sync is open on Minutes.

## Notes
- Creates or changes nothing. Four of the data's recorded calls had a prep; the meeting of each is in the calendar (`ev-umbrella-intro`, `ev-m15`, `ev-globex-standup`, `ev-m19`).
