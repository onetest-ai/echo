---
id: TC-021
title: A thread's Since date keeps older calls out, whatever its rules say, and an earlier one lets them in
priority: high
type: functional
requirements: [F-THR-14, F-THR-15, F-THR-19]
module: threads
run: agent
platform: any
tags: [threads, since, membership, pairwise]
---

# TC-021: A thread's Since date keeps older calls out, whatever its rules say, and an earlier one lets them in

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- Acme rollout's since date is 1 January of the run's year (the data)

## Test Data

| Thread | Since | Calls it holds |
|---|---|---|
| Acme rollout | 1 January of the run's year (`profile.json` since) | 6 (Acme rollout kickoff, 20 November last year, is not one) |
| Acme renewal | 20 days ago | 2 |
| Globex pilot | none: "From its first call" | 8 |
| Umbrella proposal | 30 days ago | 5 |
| Acme hackathon | 30 days ago | 1 |
| Legacy migration | 60 days ago | 1 |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `ls "$ECHOCTL_HOME/calls" \| grep acme-rollout-kickoff` to get the folder, then `echoctl project of <folder>` | `projects` is empty (the call is older than the since date of Acme rollout, whose subject rule "rollout" would otherwise take it); `suggested` lists acme-renewal and acme-rollout |
| 2 | Navigate to `{{base_url}}/#project/acme-rollout/calls` | The header says "6 calls · since" and the date; the tab "Calls" shows 6; no row "Acme rollout kickoff" |
| 3 | Navigate to `{{base_url}}/#project/acme-rollout/profile` and read the row "Since" | It shows 1 January of the run's year and "· nothing older is read" |
| 4 | Navigate to `{{base_url}}/#project/globex-pilot/profile` and read the row "Since" | It reads "From its first call" |
| 5 | Read the row "Since" of the Profile of acme-renewal, umbrella-proposal, acme-hackathon and legacy-migration | A date each: 20 days ago, 30 days ago, 30 days ago and 60 days ago, with "· nothing older is read" |
| 6 | Run `echoctl project set acme-rollout --since 2025-11-01` | Succeeds |
| 7 | Navigate to `{{base_url}}/#project/acme-rollout/calls` | The tab shows 7; the new row "Acme rollout kickoff" says "its subject" and "Take out" |
| 8 | Run `echoctl project of <folder of Acme rollout kickoff>` | `projects` lists acme-rollout with `why` "subject"; `suggested` lists acme-renewal only |

## Expected Final State
Acme rollout holds 7 calls until the teardown.

## Teardown
- Run `echoctl project set acme-rollout --since <1 January of the run's year, YYYY-01-01>`; the tab "Calls" shows 6 again

## Changes
- Changes Acme rollout's since date and puts it back.

## Notes
- `project of` still suggests Acme rollout for the old call after step 1 although its since date excludes it (it shows in `suggested`). A suggestion never assigns, but it offers a thread that could not take the call. Verify at run time.
