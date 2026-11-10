---
id: TC-024
title: A suggested thread is only offered on the call; nothing is added until the user presses Add
priority: critical
type: functional
requirements: [F-THR-19, F-THR-11]
module: threads
run: agent
platform: any
tags: [threads, suggestions, membership, cli]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-024: A suggested thread is only offered on the call; nothing is added until the user presses Add

**Module:** Threads | **Priority:** Critical | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No call has been added to or taken out of a thread since the data was built

## Test Data

| Call (title) | Suggested threads, in the order of `project of` | Bar in the call header |
|---|---|---|
| Acme renewal kickoff | Acme rollout (1 of its 1 people are in its calls) | Part of Acme rollout? |
| Acme rollout kickoff | Acme renewal (3 of its 4 people), Acme rollout (4 of its 4 people) | Part of Acme renewal? |
| Internal tooling budget | Acme renewal, Acme rollout, Globex pilot, Initech hiring, Internal tooling (its subject names "Internal tooling") | Part of Acme renewal? |
| Globex weekly | Acme rollout (3 of its 3 people), Initech hiring (2 of its 3 people) | Part of Acme rollout? |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | For each call of the table run `echoctl project of <folder>` (`ls "$ECHOCTL_HOME/calls" \| grep <slug>`; slugs acme-renewal-kickoff, acme-rollout-kickoff, internal-tooling-budget, globex-weekly) | `projects` is empty and `suggested` lists the threads of the row, with their reasons |
| 2 | Navigate to `{{base_url}}/#call/<folder of Acme renewal kickoff>` | The header shows the button "+ Thread" and the bar "Part of Acme rollout?" with the reason, the buttons "Add" and "Not this" (neither pressed); no thread chip |
| 3 | Repeat for the other three calls | Each bar names the first thread of its row only |
| 4 | Navigate to `{{base_url}}/#project/acme-rollout/calls` and `#project/acme-renewal/calls` | None of the four calls is a row of either thread (Acme rollout 6 rows, Acme renewal 2) |
| 5 | Run `echoctl project of` again for the four calls | `projects` is still empty: looking at a suggestion put nothing in a thread |

## Expected Final State
No call changed threads.

## Teardown
- None: the case only reads

## Notes
- Suggestions are ordered by thread id, not by strength: "Internal tooling budget" is offered Acme renewal first (1 of its 2 people) and Internal tooling (its subject names it) last, and the bar shows only the first. Verify at run time.
- Extends `TC-014` (a rule puts a call in) to the other direction: no rule, so no thread.
