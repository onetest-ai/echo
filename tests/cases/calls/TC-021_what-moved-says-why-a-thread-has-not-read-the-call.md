---
id: TC-021
title: What moved says why a thread has not read the call yet and offers Sync now
priority: medium
type: functional
requirements: [F-MIN-05]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, what-moved]
---

# TC-021: What moved says why a thread has not read the call yet and offers Sync now

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Thread line | Button |
|---|---|---|
| Memo (`memo`) | Acme rollout: "hasn't read this call yet" | Sync now |
| Memo (`memo`) | Globex pilot: "reads this call within a few minutes" | Sync now |
| Globex pilot kickoff | Globex pilot: "hasn't read this call yet" | Sync now |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Memo, then open What moved with the header button `[data-panel-flip="moved"]` (skip the click if the right panel already shows What moved) | The What moved view in the right panel (`#r-right .mv`) has two thread lines; no count on its tab or button (neither thread has read the memo) |
| 2 | Read the line of Acme rollout | "hasn't read this call yet" and a button "Sync now" (`data-cc-sync="acme-rollout"`) |
| 3 | Read the line of Globex pilot | "reads this call within a few minutes" and a button "Sync now" (`data-cc-sync="globex-pilot"`) |
| 4 | Click the row Globex pilot kickoff | One thread line: Globex pilot "hasn't read this call yet" with "Sync now" (do not press it: it starts Claude Code) |

## Expected Final State
Globex pilot kickoff is open; no Sync now was pressed.

## Notes
- Creates or changes nothing. The other reasons ("is reading this call now…", "has nothing to read") need a running sync or a call without transcript.
