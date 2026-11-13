---
id: TC-045
title: Echo's own widgets set to a thread or a client — Threads at a glance (a client), Done this week and Recorded this week (a thread or a client) show only its threads and name it in their header, on Today by ⚙ and on a thread's or client's dashboard by the page
priority: high
type: functional
requirements: [F-TODAY-17, F-TODAY-18, F-TODAY-23, F-TODAY-28]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, dashboards, threads, clients]
---

# TC-045: Echo's own widgets set to a thread or a client

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today add core:tasks.glance --size M`; `today add core:tasks.done --size M`; `today add core:calls.week --size M`; then `target/debug/echoctl widget show core:tasks.glance` | Its `threads` every open thread with tasks (Acme rollout, Acme renewal, Globex pilot, …), no `scope` |
| 2 | `target/debug/echoctl today set core:tasks.glance client acme`; `today set core:tasks.done thread acme-rollout`; `today set core:calls.week client acme`; open `{{base_url}}/#today` | Headers "THREADS AT A GLANCE · ACME" (only Acme rollout and Acme renewal), "DONE THIS WEEK · ACME ROLLOUT" (only Acme rollout's tasks), "RECORDED THIS WEEK · ACME" (fewer calls than in step 1's `widget show core:calls.week` with no setting) |
| 3 | Edit Today; ⚙ on Done this week; set Thread to "All threads"; Done | "Period" (week), "Thread" (Acme rollout; its first option "All threads"), "Client" ("Any client"). After "All threads": the header "DONE THIS WEEK" with no thread, every thread's tasks again. Then `target/debug/echoctl today set core:tasks.done thread acme-rollout` again |
| 4 | `target/debug/echoctl today --thread acme-rollout add core:tasks.glance` | Refused: "core:tasks.glance can't go on a thread's dashboard: it has no thread setting" |
| 5 | `target/debug/echoctl today --thread acme-rollout add core:tasks.done --size M`; `today --thread acme-rollout add core:calls.week --size M --after core:tasks.done`; open `{{base_url}}/#project/acme-rollout/brief` | Over "What it is": "DONE THIS WEEK · ACME ROLLOUT" and "RECORDED THIS WEEK · ACME ROLLOUT" side by side |
| 6 | Click ✎; ⚙ on Done this week | The panel offers "Done this week" and "Recorded this week" (both "here"), no "Threads at a glance"; Done this week's settings only "Period" (the page sets its thread and client). Click Done |
| 7 | `target/debug/echoctl today --client acme add core:tasks.glance --size L`; `today --client acme add core:tasks.done --size M`; open `{{base_url}}/#client/acme/overview`; ✎, ⚙ on Done this week | Over "About Acme": "THREADS AT A GLANCE · ACME" with Acme rollout and Acme renewal only. Done this week's settings "Period" and "Thread" (no Client: the page's), Thread offering "All threads", Acme renewal, Acme rollout — no other client's thread. Done |
| 8 | `target/debug/echoctl today --thread acme-rollout reset`; `today --client acme reset`; `today reset` | `[]`, `[]`, then Today as it comes |

## Teardown
- `tests/fixtures/serve.sh down <dir>`; close the browser tab
