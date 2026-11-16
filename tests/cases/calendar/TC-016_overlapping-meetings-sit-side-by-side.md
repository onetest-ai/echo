---
id: TC-016
title: Meetings at the same time sit side by side in lanes, none hidden behind another
priority: medium
type: functional
requirements: [F-CAL-03]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, layout]
---

# TC-016: Meetings at the same time sit side by side in lanes, none hidden behind another

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today
- Viewport 1201x868

## Test Data

| Group | Blocks (ids) |
|---|---|
| Live: 7 blocks, from 10 minutes before the build to 20 minutes after it | `ev-m02`, `ev-m07`, `ev-m14`, `ev-m18`, `ev-m28`, `ev-m31`, `ev-m33` |
| To come: 11 blocks, from 37 minutes after the build (25 or 30 minutes long), and the invite not answered just before them (`ev-free-invite`, 22 minutes after the build, 14 minutes) | `ev-m04`, `ev-m08`, `ev-m16`, `ev-m20`, `ev-m29`, `ev-m32`, `ev-m34`, `ev-renewal-call`, `ev-globex-weekly`, `ev-pinned`, `ev-free-invite` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day" | The day is shown |
| 2 | Read the bounding boxes (`getBoundingClientRect`) of the 7 blocks of the first group | The 7 boxes have different `left` values; their horizontal ranges do not overlap; their widths are equal (one lane each; the lanes of the group are shared with the recorded Globex standup that ends 5 minutes after they start, so a lane is one eighth of the column, less 4 px) |
| 3 | Read the bounding boxes of the 11 blocks of the second group | 11 different `left` values; no two overlap horizontally; equal widths (one eleventh of the column, less 4 px) |
| 4 | Read the `top` of the two groups | Every block of the first group starts at the same `top` (they start at the same time); so does every block of the second group but `ev-free-invite`, whose `top` is above theirs (it starts 15 minutes earlier); the first group's blocks are above the second group's |

## Expected Final State
Both groups are drawn in lanes with nothing hidden.

## Notes
- The data puts every meeting to come but the invite at the same start, 37 minutes after the build, and the invite at 22
  minutes after it, at any hour of the build (`build_agenda` in `tests/fixtures/make_data.py`); the live ones all start
  10 minutes before it.
- Creates or changes nothing. Uses `evaluate` only to read geometry.
