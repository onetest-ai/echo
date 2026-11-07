---
id: TC-033
title: While a call is being transcribed, the call opens on Transcript and Minutes says they come after the call
priority: medium
type: functional
requirements: [F-MIN-10, F-CALLS-06]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, status]
---

# TC-033: While a call is being transcribed, the call opens on Transcript and Minutes says they come after the call

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | State |
|---|---|
| Quick check-in (`quick-check-in`) | status transcribing, `auto.due` not_ready, no people |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Quick check-in | The tab "Transcript" is selected (`aria-selected="true"`) |
| 2 | Click the tab "Minutes" | A box with the heading "Minutes come after the call" and the text "Echo writes them 15 minutes after the call. Add your own notes in the Notes tab meanwhile: the minutes are written around them." |
| 3 | Look for buttons in the box | No "Write minutes" |

## Expected Final State
Quick check-in shows that minutes come after the call.

## Notes
- Creates or changes nothing. The other reasons (Claude Code missing, plugin missing, "can't start") depend on the machine: in the data `claude` is the fake and both are present.
