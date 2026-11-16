---
id: TC-046
title: Your day, Next meeting and My work set to a thread or a client — only its threads' meetings, calls and tasks, the header naming it; Today's head still counts the whole day; on a thread's dashboard the page sets it
priority: high
type: functional
requirements: [F-TODAY-02, F-TODAY-03, F-TODAY-13, F-TODAY-23, F-TODAY-28]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, dashboards, threads, clients]
---

# TC-046: Your day, Next meeting and My work set to a thread or a client

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today`; note the head ("… · N meetings · M tasks on you") | Your day lists every thread's meetings (Globex steering, Acme weekly sync, Umbrella intro call, …); My work's cards from several threads (Acme rollout, Acme renewal, Globex pilot) |
| 2 | `target/debug/echoctl today set core:calendar.agenda client acme`; `today set core:tasks.mine thread acme-rollout`; `today set core:calendar.next client globex`; reload | Headers "YOUR DAY · ACME" (only meetings in Acme's threads — Acme weekly sync, Acme renewal follow-up, Steering prep — and calls recorded in them; no Globex or Umbrella meeting, no task lines) and "MY WORK · THIS WEEK · ACME ROLLOUT" (only "· Acme rollout" cards); Next meeting a Globex pilot meeting; the head's counts as in step 1 |
| 3 | `target/debug/echoctl today set core:calendar.agenda client ""`; reload | "YOUR DAY" again with every thread's meetings |
| 4 | `target/debug/echoctl today --thread acme-rollout add core:tasks.mine --size M`; `today --thread acme-rollout add core:calendar.next --size M --after core:tasks.mine`; `today --thread acme-rollout add core:calendar.agenda --size M`; open `{{base_url}}/#project/acme-rollout/brief` | Over "What it is": "MY WORK · THIS WEEK · ACME ROLLOUT" (Acme rollout's cards) beside Next meeting (an Acme rollout meeting, its chip "Acme rollout"); under them "YOUR DAY · ACME ROLLOUT" (Acme weekly sync, its calls, the next one marked "next") |
| 5 | Click a My work card on the dashboard; then its ✓ (Close…), Cancel; then click the Acme weekly sync line in Your day, close its card; then its ⋯ | Its details open in the right panel, the Brief staying put; ✓ opens "Close this task?" with the card's title, Cancel closes it; the line opens the meeting's card beside it (as on Today); ⋯ opens Calendar's menu for it |
| 5a | `target/debug/echoctl today set core:calendar.agenda client initech`; `today set core:tasks.mine thread initech-hiring`; open `{{base_url}}/#today` | Your day and My work set to Initech (nothing today) take no place; no "A quiet day" (the day itself isn't quiet: the head still counts its meetings and tasks) |
| 6 | `target/debug/echoctl today --thread acme-rollout reset`; `today reset` | `[]`, then Today as it comes |

## Teardown
- `tests/fixtures/serve.sh down <dir>`; close the browser tab
