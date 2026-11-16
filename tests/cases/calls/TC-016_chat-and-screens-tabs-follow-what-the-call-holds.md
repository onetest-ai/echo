---
id: TC-016
title: A call shows Chat and Screens tabs, with counts, only when it has chat or frames
priority: medium
type: functional
requirements: [F-CALLS-07, F-CALLS-14, F-CALLS-15]
module: calls
run: agent
platform: any
data: standard
tags: [calls, tabs, extras]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-016: A call shows Chat and Screens tabs, with counts, only when it has chat or frames

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Tabs in order (count) |
|---|---|
| Acme weekly sync (Today) | Minutes, Transcript, Chat (3), Screens (2) |
| Globex pilot kickoff (`globex-pilot-kickoff`) | Minutes, Transcript, Chat (1) |
| Initech planning | Minutes, Transcript |
| Memo | Minutes, Transcript |
| Quick check-in (being transcribed) | Minutes, Transcript; it opens on Transcript |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync | The tablist (`role=tab`) reads Minutes, Transcript, Chat with a count 3 (`.n`), Screens with a count 2 |
| 2 | Click the row Globex pilot kickoff | The tabs read Minutes, Transcript, Chat with count 1; no Screens |
| 3 | Click the row Initech planning | The tabs read Minutes, Transcript only |
| 4 | Click the row Memo | The tabs read Minutes, Transcript only |
| 5 | Click the row Quick check-in | The tab Transcript is selected (`aria-selected="true"`); the tabs read Minutes, Transcript only |

## Expected Final State
Quick check-in is open on Transcript.

## Notes
- Creates or changes nothing. The Prep tab (first) is checked in `prep/TC-010`. A recording call (red dot on Transcript, listed first) cannot be made here: `call.status=recording` is skipped.
