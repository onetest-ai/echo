---
id: TC-010
title: A closed stage moves the thread to "Closed · N" and its history is shown
priority: high
type: functional
requirements: [F-THR-12]
module: threads
run: agent
platform: any
tags: [threads, stage]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-010: A closed stage moves the thread to "Closed · N" and its history is shown

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot |
| Stages | Wrap-up, then Done |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/profile` | The Stage row has three groups: PURSUIT (Lead, Proposal, Negotiation), DELIVERY (Kickoff, Active, On hold, Wrap-up), CLOSED (Done, Lost, Cancelled); "Active" is pressed; the line reads "Active since <Mon D>" |
| 2 | Click "Wrap-up". Wait up to 3 s after an action for the window to redraw before reading. | "Wrap-up" is pressed; the header tag reads "Delivery · Wrap-up"; the history line starts with "Wrap-up since" and contains "Active" |
| 3 | Click "Done". Wait up to 3 s after an action for the window to redraw before reading. | "Done" is pressed; the header tag ends with "Done"; the left list under GLOBEX no longer shows a row "Globex pilot"; a button "Closed · 1" is at the end of the list |
| 4 | Click "Closed · 1" | A row "Globex pilot" is listed under it, without a stage tag |
| 5 | Run `echoctl project list` | The entry `globex-pilot` has `"stage": "done"` and `"group": "closed"` |

## Expected Final State
Globex pilot is closed (Done) and listed under "Closed · 1".

## Teardown
- Click "Active" on the Profile (or `echoctl project set globex-pilot --stage active`); confirm the row is back under GLOBEX

## Changes
- Changes the stage of `globex-pilot` twice (history keeps it).
