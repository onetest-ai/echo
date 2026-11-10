---
id: TC-020
title: A call can be in two threads, with its own reason in each, and each client page shows it under its own thread
priority: medium
type: functional
requirements: [F-THR-11, F-THR-19, F-CLIENT-03]
module: threads
run: agent
platform: any
tags: [threads, membership, calls, clients]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-020: A call can be in two threads, with its own reason in each, and each client page shows it under its own thread

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No pin or rule has been changed since the data was built

## Test Data

| Call | In thread | Reason |
|---|---|---|
| Memo | Acme rollout | added by hand |
| Memo | Globex pilot | added by hand |
| Globex pilot scoping | Acme rollout | added by hand |
| Globex pilot scoping | Globex pilot | the client's people |
| Initech planning | none | only suggested |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/calls` | The rows "Memo" and "Globex pilot scoping" are there, both "added by hand" with "Unpin" |
| 2 | Navigate to `{{base_url}}/#project/globex-pilot/calls` | The rows "Memo" ("added by hand", "Unpin") and "Globex pilot scoping" ("the client's people", "Take out") are there |
| 3 | Navigate to `{{base_url}}/#client/acme/calls` | The list holds 9 calls; the row "Globex pilot scoping" has one chip, "Acme rollout" |
| 4 | Navigate to `{{base_url}}/#client/globex/calls` | The list holds 8 calls; the row "Globex pilot scoping" has one chip, "Globex pilot" |
| 5 | Run `echoctl project of <folder of Globex pilot scoping>` (`ls "$ECHOCTL_HOME/calls" \| grep globex-pilot-scoping`) | Both threads are listed in `projects`: acme-rollout with `why` "pinned", globex-pilot with `why` "people" |
| 6 | Run `grep -l -e '"acme-rollout"' -e '"globex-pilot"' "$ECHOCTL_HOME"/calls/*globex-pilot-scoping/meta.json "$ECHOCTL_HOME"/calls/*globex-pilot-scoping/meeting.json` (quotes included: the folder name and the meeting id contain the words without them) | No file is listed: the threads are not written into the call |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- A pin does not exclude the other threads: the same call is also in Globex pilot by its rule.
