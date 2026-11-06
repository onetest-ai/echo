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
| 23:02-type, 7 blocks, same end | `ev-m02`, `ev-m07`, `ev-m14`, `ev-m18`, `ev-m28`, `ev-m31`, `ev-m33` |
| 23:50-type, 10 blocks | `ev-m04`, `ev-m08`, `ev-m16`, `ev-m20`, `ev-m29`, `ev-m32`, `ev-m34`, `ev-renewal-call`, `ev-globex-weekly`, `ev-pinned` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day" | The day is shown |
| 2 | Read the bounding boxes (`getBoundingClientRect`) of the 7 blocks of the first group | The 7 boxes have different `left` values; their horizontal ranges do not overlap; their widths are equal (one seventh of the column, less 4 px) |
| 3 | Read the bounding boxes of the 10 blocks of the second group | 10 different `left` values; no two overlap horizontally; equal widths |
| 4 | Read the `top` of the two groups | Every block of a group starts at the same `top` (they start at the same time); the first group's blocks are above the second group's |

## Expected Final State
Both groups are drawn in lanes with nothing hidden.

## Notes
- Creates or changes nothing. Uses `evaluate` only to read geometry.
