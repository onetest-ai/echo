---
id: TC-019
title: The Calls tab says why each call is in a thread: its series, the client's people, its subject or added by hand
priority: high
type: functional
requirements: [F-THR-11, F-THR-14]
module: threads
run: agent
platform: any
tags: [threads, calls, membership, pairwise]
---

# TC-019: The Calls tab says why each call is in a thread: its series, the client's people, its subject or added by hand

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No rule, pin or exclusion has been changed since the data was built

## Test Data

**Acme rollout** (`acme-rollout`, 6 calls)

| Call | Started | Reason line | Minutes | Button |
|---|---|---|---|---|
| Memo | today | added by hand | - | Unpin |
| Acme weekly sync | today | its meeting series | - | Take out |
| Acme security review | yesterday | added by hand | no minutes yet | Unpin |
| Acme weekly sync | 7 days ago | its meeting series | - | Take out |
| Globex pilot scoping | 10 days ago | added by hand | - | Unpin |
| Acme weekly sync | 14 days ago | its meeting series | - | Take out |

**Acme renewal** (`acme-renewal`, 2 calls)

| Call | Started | Reason line | Minutes | Button |
|---|---|---|---|---|
| Acme renewal negotiation | 3 days ago | its subject | - | Take out |
| Acme renewal pricing | 6 days ago | its subject | - | Take out |

**Globex pilot** (`globex-pilot`, 8 calls)

| Call | Started | Reason line | Minutes | Button |
|---|---|---|---|---|
| Globex standup | today | the client's people | no minutes yet | Take out |
| Memo | today | added by hand | - | Unpin |
| Acme demo | today | the client's people | no minutes yet | Take out |
| Globex steering | today | the client's people | - | Take out |
| Vendor call with Vandelay | yesterday | the client's people | no minutes yet | Take out |
| Globex pilot kickoff | yesterday | the client's people | - | Take out |
| Globex support triage | 6 days ago | the client's people | no minutes yet | Take out |
| Globex pilot scoping | 10 days ago | the client's people | - | Take out |

**Umbrella proposal** (`umbrella-proposal`, 5 calls)

| Call | Started | Reason line | Minutes | Button |
|---|---|---|---|---|
| Globex retro | today | the client's people | no minutes yet | Take out |
| Umbrella contract call | today | the client's people | no minutes yet | Take out |
| Umbrella intro call | today | added by hand | no minutes yet | Unpin |
| Umbrella proposal walkthrough | 5 days ago | the client's people | no minutes yet | Take out |
| Umbrella discovery | 16 days ago | the client's people | no minutes yet | Take out |

**Initech hiring** (`initech-hiring`, 3 calls)

| Call | Started | Reason line | Minutes | Button |
|---|---|---|---|---|
| Guest call 7 | yesterday | added by hand | - | Unpin |
| Imported review 2 | 2 days ago | added by hand | - | Unpin |
| Initech hiring panel | 4 days ago | its subject | - | Take out |

**Internal tooling** (`internal-tooling`, 3 calls)

| Call | Started | Reason line | Minutes | Button |
|---|---|---|---|---|
| Guest call 7 | yesterday | added by hand | - | Unpin |
| Imported review 2 | 2 days ago | added by hand | - | Unpin |
| Memo 9 | 3 days ago | added by hand | - | Unpin |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/calls`; read the count on the tab "Calls", the rows from the top and the button of every row, and look for the reason "its Teams chat" | The tab carries the count of the thread's table (6); the rows are those of the table, newest first; each row's second line has the day and time, " · " and the reason of its row, and " · no minutes yet" where the table says so; "Unpin" on a row "added by hand", "Take out" on every other row; no row says "its Teams chat" |
| 2 | Do step 1 for each other thread of the tables, one visit each: `acme-renewal`, `globex-pilot`, `umbrella-proposal`, `initech-hiring`, `internal-tooling` | The same for each thread's table (counts 2, 8, 5, 3, 3); "its Teams chat" is on no row of the six tabs: the chat rules of Globex pilot and Umbrella proposal match no call of the data |

## Expected Final State
The six Calls tabs show the rows of the tables.

## Teardown
- None: the case only reads

## Notes
- One reason is shown per call, the first rule that matches (a pin shows "added by hand").
- Umbrella proposal holds "Globex retro" by the client's people: the call has an Umbrella person in it.
- Internal tooling has no rule: all its three calls are added by hand; Initech hiring has two pinned calls and one by its subject.
- Extends `TC-008` and `TC-009` (one pinned and one rule call) to reasons by series, subject and people.
