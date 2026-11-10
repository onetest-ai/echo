---
id: TC-011
title: A prep whose threads run on different AI profiles says so and shows no In short
priority: medium
type: functional
requirements: [F-PREP-04]
module: prep
run: agent
platform: any
data: standard
tags: [prep, error]
---

# TC-011: A prep whose threads run on different AI profiles says so and shows no In short

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Prep error |
|---|---|
| Globex standup (`globex-standup`) | its threads run on different AI profiles: no summary, the facts only |
| Acme go-live check (`acme-go-live-check`) | the same |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Globex standup, then the tab "Prep" | The prep shows the grey text "its threads run on different AI profiles: no summary, the facts only" |
| 2 | Look for the headings of the prep | No "In short", no "Expected from you", no "Who's coming"; the footer says "Written HH:MM" without a model name; no "Write it again" |
| 3 | Click the row Acme go-live check | The same text and footer |

## Expected Final State
Acme go-live check's prep shows only its error.

## Notes
- Creates or changes nothing. Seen on the data (2026-11-06): the same kind of prep of a meeting that is not recorded (for example `ev-m12` Analyst roundtable) cannot be shown from its calendar card: see `TC-013`.
