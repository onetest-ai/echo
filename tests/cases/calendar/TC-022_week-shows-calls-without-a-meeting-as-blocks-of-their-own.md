---
id: TC-022
title: The week shows a recorded call that has no meeting as a block of its own that opens the call
priority: medium
type: functional
requirements: [F-CAL-04, F-CAL-01, F-CAL-02]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, week, calls]
---

# TC-022: The week shows a recorded call that has no meeting as a block of its own that opens the call

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today

## Test Data

| Field | Value |
|---|---|
| Yesterday's calls without a meeting | Umbrella lead notes 1 (08:30), Acme security review (11:00), Globex pilot kickoff (14:00), Guest call 7 (14:30), Vendor call with Vandelay (16:00) |
| Today's calls without a meeting | Recording (18:12-type), Memo (21:12-type), Quick check-in (22:42-type) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar` and wait for "Week" | The URL becomes `#calendar/week`; the title reads "D Mon to D Mon YYYY" over 7 days (yesterday to five days ahead); "Week" is pressed; the banner says "Showing today's meetings only." |
| 2 | Read the blocks of yesterday's column | Five call blocks (`.ev.call`): the titles above, each with its start time |
| 3 | Read the call blocks of today's column | "Recording", "Memo" and "Quick check-in" are blocks (`.ev.call`); the meetings that have a call (Acme weekly sync…) are `.ev.recorded`, not doubled as call blocks |
| 4 | Click the block "Globex pilot kickoff" of yesterday | The URL becomes `#call/<folder>` and the call opens in Calls |
| 5 | Navigate to `{{base_url}}/#calendar/week`, click "Later" (›) | The title moves forward by 7 days; the text "Nothing on the calendar these days." is shown |
| 6 | Click "Earlier" (‹) twice, then "Today" | Earlier shows call blocks of earlier days (for example "Acme renewal negotiation"); "Today" brings the first range back |

## Expected Final State
The week shows today's range again.

## Notes
- Creates or changes nothing. Only the agenda (today) is available without Microsoft 365, so other days hold recorded calls only.
