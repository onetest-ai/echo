---
id: TC-022
title: A call taken out by hand stays out of its thread, whatever the rules say, and the tab and the profile count it
priority: medium
type: functional
requirements: [F-THR-11, F-THR-14, F-THR-19]
module: threads
run: agent
platform: any
tags: [threads, membership, exclusions, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-022: A call taken out by hand stays out of its thread, whatever the rules say, and the tab and the profile count it

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No rule, pin or exclusion has been changed since the data was built

## Test Data

| Thread | Call taken out by hand | Footer on the Calls tab | Profile line under the row Calls |
|---|---|---|---|
| Acme renewal | Acme renewal kickoff | 1 call taken out by hand. | 2 calls match · 1 taken out by hand |
| Globex pilot | Partner sync with Vandelay | 1 call taken out by hand. | 8 calls match · 1 pinned, 1 taken out by hand |
| Legacy migration | Coffee chat | 1 call taken out by hand. | 1 call match · 1 pinned, 1 taken out by hand |
| Acme hackathon | Recording | 1 call taken out by hand. | 1 call match · 1 pinned, 1 taken out by hand |
| Acme rollout | - | no footer | 6 calls match · 3 pinned |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | For each of Acme renewal, Globex pilot, Legacy migration and Acme hackathon navigate to `{{base_url}}/#project/<thread id>/calls` | Below the rows the text of the Footer column; the taken-out call is not among the rows |
| 2 | Navigate to `{{base_url}}/#project/acme-rollout/calls` | No footer "taken out by hand" under the rows |
| 3 | For each of the five threads navigate to `#project/<thread id>/profile` and read the line under the row "Calls" | The Profile line of the table |
| 4 | Run `echoctl project of <folder of Partner sync with Vandelay>` (`ls "$ECHOCTL_HOME/calls" \| grep partner-sync-with-vandelay`) | `projects` is empty; `suggested` is empty too (the call was taken out of Globex pilot) |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Acme hackathon's exclusion is the call of today called "Recording"; the pin on "Acme hackathon planning" stays.
- The Profile line is "1 call match" in the singular (the word "match" is not made plural): correct as shown, check the grammar at run time.
