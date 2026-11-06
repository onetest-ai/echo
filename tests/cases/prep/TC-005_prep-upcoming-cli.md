---
id: TC-005
title: echoctl prep --upcoming lists today's meetings and --show refuses a missing prep
priority: medium
type: functional
requirements: [F-PREP-08]
module: prep
run: agent
platform: any
tags: [prep, cli]
---

# TC-005: echoctl prep --upcoming lists today's meetings and --show refuses a missing prep

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Run before 22:00 local: the data puts the two meetings 1 h and 2.5 h ahead but keeps them inside today (never later than 23:50). Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Command | `echoctl prep --upcoming` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl prep --upcoming` | Exit code 0; JSON `meetings` with 2 entries: "Globex design review" (`names` ["Kim Park"]) and "Acme demo" (`names` ["Dana Lee"]); each has `prep: null`, `writing: false`, `threads: []`; no "Industry webinar" |
| 2 | Take the `id` of "Globex design review" from the output (a hash, not `ev-globex-review`) and run `echoctl prep --show <id>` | Exit code 1 and the message `Error: no prep "<id>"` |
| 3 | Navigate to `{{base_url}}/#calls` and click "Coming up today" | The window lists the same two meetings |

## Expected Final State
The CLI and the Coming up today list agree on two meetings, neither with a prep.

## Teardown
- Click "Coming up today" to fold it

## Notes
- Creates or changes nothing.
