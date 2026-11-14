---
id: TC-048
title: Dashboards of their own beside every thread's Brief and client's Overview — added with +, shared by every page of that kind with each page's data, renamed, moved, removed; the CLI does the same
priority: high
type: functional
requirements: [F-TODAY-29]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, dashboards, threads, clients]
---

# TC-048: Dashboards of their own

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install table-demo --yes`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#client/acme/overview` | Tabs: Overview, +, Calls, Tasks, People, Profile; Overview selected and as before |
| 2 | Click +; type "Staffing"; Enter | A tab "Staffing" after Overview, selected; the hash `#client/acme/b-staffing`; the page: "Staffing", "On every client, each showing its own", and on that same line the dashboard's ✎ with ⋯ last (no line of its own for ✎); the page runs the pane's full width |
| 3 | Click ✎; in Add a widget add "Roles to fill"; Done | "Roles to fill" on the page, its note "client acme" |
| 4 | Open `{{base_url}}/#client/globex/b-staffing` | Globex has the tab "Staffing" too, with "Roles to fill" and its note "client globex" (one layout, each client's data) |
| 5 | `target/debug/echoctl dashboard add --client Delivery`; wait about 10 seconds | A tab "Delivery" after Staffing, without a reload |
| 6 | On Delivery: ⋯ → Move left | Tabs: Overview, Delivery, Staffing, +; Move left is greyed on Delivery now (first) |
| 7 | On Staffing: ⋯ → Rename; type "People & roles"; Enter | The tab and the head read "People & roles"; the hash stays `…/b-staffing` |
| 8 | ⋯ → Remove…; Cancel; ⋯ → Remove… → Remove | First a question "Remove People & roles from every client? Its widgets go with it." and Cancel keeps it; then the tab goes and the Overview opens; `target/debug/echoctl dashboard list --client` lists only Delivery |
| 9 | `target/debug/echoctl dashboard add --thread Hiring`; `target/debug/echoctl today --thread acme-rollout --board hiring add plugin:table-demo:positions --size S`; open `{{base_url}}/#project/acme-rollout/b-hiring`; reload | The thread's tabs: Brief, Hiring, +, Work, …; Hiring shows "Open positions" with 3 (the thread's count); after the reload the same page |
| 10 | `target/debug/echoctl today --client acme --board nope` | Refused: `no client dashboard "nope": \`echoctl dashboard list --client\` names them` |
| 11 | `target/debug/echoctl dashboard remove --client delivery`; `target/debug/echoctl dashboard remove --thread hiring` | `[]` each; the tabs gone on the next refresh |
