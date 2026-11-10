---
id: TC-027
title: Threads at a glance (a traffic light a thread — done this week, to do, waiting — and its total, busiest first) and Done this week (how many since Monday, the latest at M), put on Today with echoctl today; a thread's name opens its Work, a done task its details
priority: medium
type: functional
requirements: [F-TODAY-17]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, tasks, threads]
---

# TC-027: Threads at a glance and Done this week

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl widget show core:tasks.glance` | JSON: `since` (this week's Monday), `threads` (each open thread with tasks: `done` this week, `todo`, `waiting`, `total`, busiest first), `done` (this week's done tasks, newest first, each with its thread) |
| 2 | `target/debug/echoctl today add core:tasks.glance --size M --above core:needs.now`, `today add core:tasks.done --size M --after core:tasks.glance`; open `{{base_url}}/#today` | On top "Threads at a glance": a line a thread (5 at M), its name, a bar — green done this week, amber to do, red waiting, each as wide as its count — and the total; under them the key ("done this week", "to do", "waiting") on one line each; beside it "Done this week": the number as in step 1 and "tasks since Monday", up to 4 of them with their thread |
| 3 | Hover a bar's segment | It says its count ("2 done this week", "16 to do", "1 waiting") |
| 4 | Click a thread's name | Its Work (`#project/<id>/work`) |
| 5 | Back on Today, click a done task | Its details on the right |
| 6 | `target/debug/echoctl today size core:tasks.glance L`, `today size core:tasks.done S`; reload | At L up to 10 threads ("N more in Threads" when there are more); at S Done this week shows only the number |
| 7 | `target/debug/echoctl today reset`; reload | Neither widget on Today (not in the default) |

## Notes
- The counts are `glance::view` (Rust test: done this week / to do / waiting per thread, last week's done not counted).
