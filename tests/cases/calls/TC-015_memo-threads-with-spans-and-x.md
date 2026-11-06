---
id: TC-015
title: A memo's thread chips show the time they were found in, and × takes one out
priority: high
type: functional
requirements: [F-CALLS-10, F-CALLS-09]
module: calls
run: agent
platform: any
data: standard
tags: [calls, memo, threads]
---

# TC-015: A memo's thread chips show the time they were found in, and × takes one out

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Field | Value |
|---|---|
| Call | Memo (`memo`): 12 s, pinned to two threads |
| Chips | Acme rollout 0:00–0:06 · Globex pilot 0:06–0:12 |
| Command | `echoctl project of <the memo's folder>` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Memo | The header has two thread chips: "Acme rollout" followed by "0:00–0:06", and "Globex pilot" followed by "0:06–0:12" |
| 2 | Read the `title` of the chip button "Acme rollout" and the `aria-label` of the × next to it | "Found from what you said"; "Not Acme rollout" |
| 3 | Run `echoctl project of <folder>` | JSON lists acme-rollout and globex-pilot as the call's threads |
| 4 | Click the × of the chip "Globex pilot" | The chip "Globex pilot" disappears; "Acme rollout 0:00–0:06" stays |
| 5 | Run `echoctl project of <folder>` again | globex-pilot is no longer among the call's threads; acme-rollout is |

## Expected Final State
The memo is in Acme rollout only.

## Teardown
- Run `echoctl project reset globex-pilot <the memo's folder>` (clears the exclusion the × made), then reload `{{base_url}}/#calls`

## Notes
- Changes thread membership only (no Claude Code). The pairwise call.kind × call.threads group also puts a memo with several / one / none / a suggested thread in the data: `Memo 4` (none), `Memo 9` (one, Internal tooling), `Umbrella lead notes 1` (suggestion).
