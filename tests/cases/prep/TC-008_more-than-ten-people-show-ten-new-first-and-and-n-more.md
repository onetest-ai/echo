---
id: TC-008
title: Who's coming shows ten people, the new ones first, then "and N more"
priority: medium
type: functional
requirements: [F-PREP-04]
module: prep
run: agent
platform: any
data: standard
tags: [prep, people]
---

# TC-008: Who's coming shows ten people, the new ones first, then "and N more"

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)
- A prep's id is the one in `<dir>/manifest.json` → `preps` (stable for a given meeting id); Coming up today rows are `[data-up="<prep id>"]`

## Test Data

| Field | Value |
|---|---|
| Meeting | Globex sponsor update (12 people) |
| Order | Ivan Petrov (new) · Dana Lee, Alex Chen, Lena Vogel (3 calls) · Omar Haddad, Ola Svensson, Priya Nair, Kim Park (2 calls) · Rita Gomez, Yuki Tanaka (1 call) |
| Not listed | Tom Becker, Noor Aziz → "and 2 more" |
| Meeting with 4 people | Umbrella intro call (call), all four listed, no "and N more" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync", click "Coming up today" and then the row "Globex sponsor update" | The prep is open |
| 2 | Read the side column "Who's coming" | Ten rows in the order of Test Data; the first has the chip "new"; the others show "3 calls", "2 calls" or "1 call" |
| 3 | Read the row of Dana Lee | "Dana Lee" with "· delivery lead" and "3 calls" |
| 4 | Read the line after the ten rows | "and 2 more" |
| 5 | Click the row "Umbrella intro call" in the calls list, then the tab "Prep" | "Who's coming" lists Ivan Petrov first (chip "new"), then Dana Lee, Omar Haddad and Rita Gomez; no "and N more" |

## Expected Final State
Who's coming is shown for a prep with 12 people and one with 4.

## Teardown
- Click "Coming up today" to fold it

## Notes
- Creates or changes nothing.
