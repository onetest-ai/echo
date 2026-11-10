---
id: TC-027
title: A call whose automatic minutes are ahead shows the time left, a ring, why it moved, Write now and Not yet
priority: high
type: functional
requirements: [F-MIN-07]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, auto]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-027: A call whose automatic minutes are ahead shows the time left, a ring, why it moved, Write now and Not yet

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)

## Test Data

| Call | State |
|---|---|
| Globex standup (`globex-standup`) | `auto.due` at: 15 minutes after the build, moved by the user's notes (their last edit at the build time) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Globex standup, then the tab "Minutes" | A box with a heading "Minutes in N min (HH:MM)" where N is from 1 to 15 |
| 2 | Read the ring and the text | `.mn-ring` shows the number N and "min"; the text "Echo writes them 15 minutes after the call or your last note, so what you add in My notes goes in." |
| 3 | Read the line about why it moved | "Moved: you edited your notes at HH:MM." |
| 4 | Read the model line and the buttons | "Claude Code writes them with Claude · work (sonnet), with your Claude account."; the select "Model for these minutes"; the buttons "Write now" and "Not yet ▾" (do not press Write now); "Or in a Claude Code session:" `/echo:process-call <folder>` with a Copy button |
| 5 | Read the row Globex standup in the list | It carries "⏱ N min" (the same N) |

## Expected Final State
Globex standup shows its countdown.

## Notes
- Creates or changes nothing. Write now would start Claude Code (the fake): not pressed. N shrinks by a minute every minute; run within 15 minutes of the build, or the call is due and reads "Starting the minutes…" (`TC-031`).
