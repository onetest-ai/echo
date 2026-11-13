---
id: TC-029
title: The Client filter narrows Thread to the client's threads and every pile to their tasks; another client lets the thread go; All clients brings everything back
priority: medium
type: functional
requirements: [F-TASK-04]
module: tasks
run: agent
platform: any
data: standard
tags: [tasks, filters, clients]
---

# TC-029: The Client filter narrows threads and piles

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; the browser 1440 × 1000

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#tasks`; note the piles' counts; open the "Client" filter (`[aria-label="Client"]`, before "Thread") | Its options "All clients", Initech, Acme, Globex, Umbrella, Wonka, Cyberdyne (the clients with a thread here) |
| 2 | Choose "Acme" | The chip reads "Acme ▾"; the Thread filter reads "All of Acme's ▾" and offers only "Acme renewal · …" and "Acme rollout · …"; every card's thread is Acme rollout or Acme renewal; the piles' counts smaller than in step 1 |
| 3 | In Thread choose Acme renewal | Only Acme renewal's cards |
| 4 | In Client choose "Globex" | Thread back to "All of Globex's" (Acme renewal let go); only Globex pilot's cards |
| 5 | In Client choose "All clients" | Thread "All threads"; the piles' counts as in step 1 |

## Teardown
- `tests/fixtures/serve.sh down <dir>`; close the browser tab
