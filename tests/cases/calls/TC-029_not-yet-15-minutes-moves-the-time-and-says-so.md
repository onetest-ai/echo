---
id: TC-029
title: Not yet › 15 minutes more moves the time by 15 minutes and says you said not yet
priority: medium
type: functional
requirements: [F-MIN-07]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, auto]
---

# TC-029: Not yet › 15 minutes more moves the time by 15 minutes and says you said not yet

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)

## Test Data

| Call | Choice |
|---|---|
| Globex standup | Not yet ▾ › "15 minutes more" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Globex standup, then the tab "Minutes" | The heading reads "Minutes in N min (HH:MM)"; note HH:MM as T0 |
| 2 | Click "Not yet ▾" then "15 minutes more" | The heading is "Minutes in M min (HH:MM)" where HH:MM is T0 plus 15 minutes and M is about N plus 15 |
| 3 | Read the line about why it moved | "Moved: you said not yet." |

## Expected Final State
The minutes are due 15 minutes later than before.

## Teardown
- Click "Not yet ▾" › "Until I say — I'll click Write minutes", then "Write them automatically again" (back to the original time and the note-edit line)

## Notes
- Does not start Claude Code.
