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
auto: [tests/auto/specs/S-10.spec.mjs]
---

# TC-005: echoctl prep --upcoming lists today's meetings and --show refuses a missing prep

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- The data puts the meetings to come 1 h, 1.5 h and 2.5 h ahead; built after about 20:20 the later ones start sooner, in the same order and over by 23:59, the accepted ones still 30 min (the unanswered Industry webinar then shorter); built after about 21:50 all three start sooner, and at 22:50 Globex design review starts 8 minutes after the build (tests/fixtures/README.md, #364). Every `echoctl` command runs after `. <dir>/env` (the run's env file)

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
