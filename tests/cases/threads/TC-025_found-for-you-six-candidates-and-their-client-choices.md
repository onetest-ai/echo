---
id: TC-025
title: Found for you offers six candidate threads with a client choice: none, an existing client, a new one from the domain
priority: high
type: functional
requirements: [F-THR-18, F-THR-03]
module: threads
run: agent
platform: any
tags: [threads, found, client, org]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-025: Found for you offers six candidate threads with a client choice: none, an existing client, a new one from the domain

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- Nobody has accepted or rejected a candidate since the data was built

## Test Data

| Card name | Why | Organisation line | Client selected | Other client option | Calls listed |
|---|---|---|---|---|---|
| Acme calls | 3 calls mostly with people from acme.example | - | Acme | - | Acme go-live check, Acme design review, Acme renewal kickoff |
| Northwind Analysts calls | 3 calls mostly with people from northwind.example | With Northwind Analysts: its counterpart | No client | - | Globex design review, Globex data review, Analyst briefing |
| Umbrella lead notes 1 | a meeting that recurs: 3 calls | - | No client | - | Umbrella lead notes 1, Umbrella lead notes 8, Umbrella lead notes 3 |
| Initrode calls | 2 calls mostly with people from initrode.example | With Initrode (initrode.example) — a new organisation, not a client: its counterpart | No client | New client: Initrode (initrode.example) is offered | Initrode demo, Initrode intro |
| Duff calls | 2 calls mostly with people from duff.example | - | Duff | - | Duff follow-up, Duff discovery |
| Design guild | a meeting that recurs: 2 calls | - | No client | - | Design guild, Design guild |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#projects` and click "Found for you" (`[data-pj="found"]`) | The route ends with `#found`; the heading "Found for you" and the six cards of the table, each with the buttons "Accept" and "Reject"; the badge on the button was 6 |
| 2 | Read each card: the name box, the why line, the organisation line, the Client select and the calls | They are those of the table; the Client select of "Acme calls" shows "Acme" and of "Duff calls" "Duff", the others "No client"; the card "Initrode calls" lists "New client: Initrode (initrode.example)" among its options and the card "Northwind Analysts calls" says it is with a known organisation |
| 3 | Open the select "Add to a thread…" of one card | It lists the fourteen open threads (Acme renewal, Acme rollout, Cyberdyne audit, Cyberdyne data room, Cyberdyne training, Globex pilot, Initech hiring, Umbrella lead, Umbrella proposal, Wonka factory tour, Wonka loyalty app, Wonka packaging, Wonka shop fit, Internal tooling), not the three closed ones |
| 4 | Read the top of the page | A row with the box "New thread name", the select "Client" and the button "Create" (none pressed) |
| 5 | Run `echoctl project list` and count the threads | Seventeen: neither Accept nor Reject was pressed |

## Expected Final State
The six cards are shown and nothing was accepted or rejected.

## Teardown
- None: the case only reads

## Notes
- Never press Accept or Reject: they make or hide a thread for good. `TC-017` is the empty state.
- The fixture lists Vandelay as the known organisation of this set; the window offers Northwind Analysts (the card "Northwind Analysts calls").
