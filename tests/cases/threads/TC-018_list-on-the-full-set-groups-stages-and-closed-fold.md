---
id: TC-018
title: The thread list groups seventeen threads by client, outside clients first, the user's company next, then no client, and folds the closed ones
priority: high
type: functional
requirements: [F-THR-01, F-THR-02, F-THR-03]
module: threads
run: agent
platform: any
tags: [threads, list, stage, client]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-018: The thread list groups seventeen threads by client, outside clients first, the user's company next, then no client, and folds the closed ones

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- The Threads list is not folded by hand (a fresh browser profile)

## Test Data

| Group | Thread | Calls | Stage tag | Last call (day shown) |
|---|---|---|---|---|
| Acme | Acme renewal | 2 calls | Negotiation | 3 days ago |
| Acme | Acme rollout | 6 calls | Active | today |
| Globex | Globex pilot | 8 calls | Kickoff | today |
| Umbrella | Umbrella lead | 0 calls | Lead | no last call |
| Umbrella | Umbrella proposal | 5 calls | Proposal | today |
| Tyrell | - | - | - | no thread rows |
| Duff | - | - | - | no thread rows |
| Wonka | Wonka factory tour | 0 calls | Active | no last call |
| Wonka | Wonka loyalty app | 0 calls | Active | no last call |
| Wonka | Wonka packaging | 0 calls | Active | no last call |
| Wonka | Wonka shop fit | 0 calls | Active | no last call |
| Cyberdyne | Cyberdyne audit | 0 calls | Active | no last call |
| Cyberdyne | Cyberdyne data room | 0 calls | Active | no last call |
| Cyberdyne | Cyberdyne training | 0 calls | Active | no last call |
| Initech · your company | Initech hiring | 3 calls | Wrap-up | yesterday |
| No client | Internal tooling | 3 calls | On hold | yesterday |

Closed · 3 (folded; a row has no stage tag):

| Thread | Calls | Stage | Client |
|---|---|---|---|
| Acme hackathon | 1 call | Cancelled | Acme |
| Legacy migration | 1 call | Done | Soylent (removed) |
| Tyrell bid | 1 call | Lost | Tyrell |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#projects` and wait for the heading "Threads" | The list shows the button "Found for you" with the badge 6, then the groups in this order: Acme, Globex, Umbrella, Tyrell, Duff, Wonka, Cyberdyne, "Initech · your company", "No client", then the fold "Closed · 3" |
| 2 | Read the rows of each group (`[data-pj="open"]`) | The rows of the first table, each with its name, "N calls · day of the last call" and its stage tag; Acme has Acme renewal then Acme rollout, Umbrella has Umbrella lead then Umbrella proposal; Tyrell and Duff have no row and no fold arrow |
| 3 | Read the Threads activity button in the activity bar | It carries the badge 6, the same number as "Found for you" |
| 4 | Click "Closed · 3" | The fold opens (`aria-expanded` true) with Acme hackathon, Legacy migration and Tyrell bid, each with its call count and no stage tag; the three rows are those of the second table |
| 5 | Click the arrow of the group "Acme" (title "Hide its threads") | Acme's rows are hidden and the group shows the number 2; click again to show them |
| 6 | Click the name "Tyrell" (`[data-pj="client"][data-id="tyrell"]`) | The route is `#client/tyrell/overview` |

## Expected Final State
The list shows all seventeen threads; Acme is unfolded; the closed fold is open.

## Teardown
- Click "Closed · 3" so the fold is closed again; check Acme is unfolded

## Changes
- Folds a client group and a closed fold, both remembered in the browser only.

## Notes
- Covers thread.stage (one thread per stage), thread.client (a client, the user's company, none, a client removed after its thread was made: Legacy migration is closed, so it is in the fold, not under "No client").
- Extends `TC-001`, which uses the two threads of the small data.
