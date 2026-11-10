---
id: TC-015
title: The Overview of Acme shows a card per thread: stories done, task piles, late, decisions, risks and questions, the closed one last
priority: high
type: functional
requirements: [F-CLIENT-02]
module: clients
run: agent
platform: any
tags: [clients, overview, cards, pairwise]
---

# TC-015: The Overview of Acme shows a card per thread: stories done, task piles, late, decisions, risks and questions, the closed one last

**Module:** Clients | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task, decision, risk or question has been changed since the data was built

## Test Data

| Card (in this order) | Tag | Calls | Stories | Task counts | Late | Decided | Risks | Open questions |
|---|---|---|---|---|---|---|---|---|
| Acme renewal | Negotiation | 2 calls · last … | 0 of 2 stories done | 3 I owe, 1 owed to me, 1 waiting, 1 nobody named | - | One help desk queue, Freeze changes in December | - | Which licences do we keep? |
| Acme rollout | Active | 6 calls · last … | 1 of 4 stories done | 3 I owe, 3 nobody named | 2 late (red) | Go live on a Tuesday, The pilot starts with the Berlin office | The access list arrives late (likelihood high · impact medium, "no response yet") | - |
| Acme hackathon (closed card, last) | Cancelled | 1 call · last … | - | No open tasks | - | - | - | - |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#client/acme/overview` and wait for the text "About Acme" | The section "Threads" holds three cards: Acme renewal, Acme rollout, then Acme hackathon, whose card is marked closed and comes last |
| 2 | Read each card against the table | Each shows its stage tag, its calls and last-call day, the progress bar with "N of M stories done" where there are stories, the task counts (only the piles that are not empty, among I owe, owed to me, waiting, nobody named), "N late" in red, and the blocks Decided, Risks and Open questions where they hold something |
| 3 | Read the Risks block of Acme rollout | The risk reads "no response yet" below it (no response is the weakest state) and the levels "likelihood high · impact medium" |
| 4 | Click "The access list arrives late" and wait for the right panel's tab "Details" (`[data-panel-show="work-item"][aria-selected="true"]`) (the panel may show its last tab, Inbox, for a moment) | The route is `#project/acme-rollout/work` and the right panel is on its tab "Details", showing R001 |
| 5 | Navigate back to `{{base_url}}/#client/acme/overview` and click the name "Acme hackathon" | The thread Acme hackathon opens (the route starts with `#project/acme-hackathon/`) |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- The "others" pile is not counted on a card (only the four piles the user acts on).
- Stories counted are those that are not merged, deleted or cancelled (Acme rollout S001 to S004, one done).
- Extends `TC-002` (one thread card on the small data).
