---
id: TC-026
title: The Profile shows each thread's rules as tokens with the count of calls that match, pinned and taken out
priority: medium
type: functional
requirements: [F-THR-14, F-THR-19]
module: threads
run: agent
platform: any
tags: [threads, profile, rules, membership, pairwise]
---

# TC-026: The Profile shows each thread's rules as tokens with the count of calls that match, pinned and taken out

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No rule, pin or exclusion has been changed since the data was built

## Test Data

| Thread | Thread id | Rule tokens, in order | Line under them |
|---|---|---|---|
| Acme rollout | acme-rollout | series “Acme weekly sync”, subject “rollout” | 6 calls match · 3 pinned |
| Acme renewal | acme-renewal | subject “renewal”, series “Acme renewal weekly” | 2 calls match · 1 taken out by hand |
| Globex pilot | globex-pilot | Globex people ≥ half, from a Teams chat | 8 calls match · 1 pinned, 1 taken out by hand |
| Umbrella proposal | umbrella-proposal | people from umbrella.example ≥ half, subject “proposal”, from a Teams chat | 5 calls match · 1 pinned |
| Tyrell bid | tyrell-bid | Tyrell people ≥ half, series “Tyrell weekly”, from a Teams chat, subject “tyrell” | 1 call match |
| Legacy migration | legacy-migration | people from soylent.example ≥ half | 1 call match · 1 pinned, 1 taken out by hand |
| Umbrella lead | umbrella-lead | none | 0 calls match |
| Internal tooling | internal-tooling | none | 3 calls match · 3 pinned |
| Initech hiring | initech-hiring | subject “hiring”, series “Hiring sync” | 3 calls match · 2 pinned |
| Acme hackathon | acme-hackathon | none | 1 call match · 1 pinned, 1 taken out by hand |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/profile`; read the row "Calls" (its tokens and the line under them) and run `echoctl project show acme-rollout` | The tokens are the Rule tokens of the thread's row, in that order, each with a "×", then "+ Add" (a thread with none has only "+ Add"); the line under them is the last column of the row; the CLI's `rules`, `call_count`, the number of `pins` and of `exclusions` agree with the tokens and the line |
| 2 | Do step 1 for each other thread of the table, one visit each (any order) | The same for each row |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- The Teams chat rules match no call on the data; a token "from a Teams chat" has no name.
- Covers thread.rules (none, series, people of the client or of its own domains, chat, subject, several) against pins and exclusions and the stage (Pursuit, Delivery, Closed).
