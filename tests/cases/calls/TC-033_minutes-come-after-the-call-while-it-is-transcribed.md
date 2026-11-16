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
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-033: While a call is being transcribed, the call opens on Transcript and Minutes says they come after the call

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | State |
|---|---|
| Quick check-in (`quick-check-in`) | status transcribing, `auto.due` not_ready, no people |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Load `{{base_url}}/` fully (a reload, not a hash change), then navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync" and click the row Quick check-in | The tab "Transcript" is selected (`aria-selected="true"`) and the URL ends with `/transcript` (the window wrote the tab it chose into the URL) |
| 2 | Click the tab "Minutes" | A box with the heading "Minutes come after the call" and the text "Echo writes them 15 minutes after the call. Add your own notes in the Notes tab meanwhile: the minutes are written around them." |
| 3 | Look for buttons in the box | No "Write minutes" |

## Expected Final State
Quick check-in shows that minutes come after the call.

## Notes
- The window picks Transcript for a call being transcribed once per page load (`S.autoTab`); after that the call opens
  on the tab last used, so a stage that opened Quick check-in earlier in the scenario would leave it on Minutes. The
  full load in step 1 makes the case independent of the stages before it.
  In S-23 the stage before it (`TC-016`, step 5) has just opened Quick check-in for the first time since the page load,
  from the Minutes tab: the scenario skips the full load and checks step 1's result on that view.
- Steps 1 and 2 check #148: the window once left the URL at `#call/<folder>/minutes` while it showed Transcript, so
  the click on Minutes asked for the URL it already had and nothing changed.
- Creates or changes nothing. The other reasons (Claude Code missing, plugin missing, "can't start") depend on the machine: in the data `claude` is the fake and both are present.
