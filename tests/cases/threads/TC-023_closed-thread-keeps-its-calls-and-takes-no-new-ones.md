---
id: TC-023
title: A closed thread keeps the calls that came before it closed and takes none that start after
priority: high
type: functional
requirements: [F-THR-12, F-THR-19]
module: threads
run: agent
platform: any
tags: [threads, stage, membership, closed, pairwise]
---

# TC-023: A closed thread keeps the calls that came before it closed and takes none that start after

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No stage, rule or pin has been changed since the data was built

## Test Data

| Thread | Stage and when | Call kept | Call it does not take | Why it would be in |
|---|---|---|---|---|
| Tyrell bid | Lost, 5 days ago | Tyrell bid review (before it was lost) | Tyrell follow-up (2 days ago) | Tyrell people, the series "Tyrell weekly", the subject "tyrell" |
| Legacy migration | Done, 10 days ago | Legacy migration retro (added by hand) | Soylent check-in (3 days ago) | its rule: people from soylent.example |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `echoctl project of <folder>` for `ls "$ECHOCTL_HOME/calls" \| grep tyrell-follow-up` and for `grep soylent-check-in` | `projects` is empty and `suggested` is empty for both: a closed thread takes no new call, even a matching one, and is not suggested |
| 2 | Run `echoctl project show tyrell-bid` | `"call_count": 1`; the call Tyrell bid review has `"why": "people"`; `"group": "closed"`; its `stages` end with `lost` |
| 3 | Navigate to `{{base_url}}/#project/tyrell-bid/calls` | One row: "Tyrell bid review" with "the client's people"; no row "Tyrell follow-up" |
| 4 | Navigate to `{{base_url}}/#project/legacy-migration/calls` | One row: "Legacy migration retro", "added by hand" and "Unpin"; no row "Soylent check-in" |
| 5 | Navigate to `{{base_url}}/#client/tyrell/calls` | One call, "Tyrell bid review", with the chip "Tyrell bid" |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Extends `TC-010` (closing a thread on the small data) to two threads that were closed by a lost bid and by a finished project.
