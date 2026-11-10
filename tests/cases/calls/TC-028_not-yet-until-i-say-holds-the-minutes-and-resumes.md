---
id: TC-028
title: Not yet › Until I say holds the minutes; Write them automatically again brings the countdown back
priority: high
type: functional
requirements: [F-MIN-07, F-MIN-08]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, auto]
---

# TC-028: Not yet › Until I say holds the minutes; Write them automatically again brings the countdown back

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)

## Test Data

| Call | Choice |
|---|---|
| Globex standup | Not yet ▾ › "Until I say — I'll click Write minutes", then "Write them automatically again" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Globex standup, then the tab "Minutes" | The heading reads "Minutes in N min (HH:MM)" |
| 2 | Click "Not yet ▾" | A menu with the items "15 minutes more" and "Until I say — I'll click Write minutes" |
| 3 | Click "Until I say — I'll click Write minutes" | The heading becomes "Minutes when you say" with the text "You asked for these later: Echo won't write them by itself."; the buttons "Write them automatically again" and "Write minutes"; no ring |
| 4 | Read the row Globex standup in the list | It no longer carries a "⏱" chip |
| 5 | Click "Write them automatically again" | The heading is "Minutes in N min (HH:MM)" again, with "Moved: you edited your notes at HH:MM."; the row carries the chip again |

## Expected Final State
The countdown is back as it was.

## Notes
- Changes the call's minutes timing only (a file in the call folder); step 5 undoes it. Does not start Claude Code.
