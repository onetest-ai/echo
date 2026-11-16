---
id: TC-050
title: Filters — a widget that gathers the settings of the widgets beside it, one control per setting they share, set on all of them at once; the CLI does the same
priority: high
type: functional
requirements: [F-TODAY-31]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, dashboards, filters]
---

# TC-050: Filters on a dashboard

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install table-demo --yes`
- Acme's Overview holds "Roles to fill", "Open positions", "Done this week" and "Threads at a glance": `target/debug/echoctl today --client acme add plugin:table-demo:roles`, the same for `plugin:table-demo:positions`, `core:tasks.done`, `core:tasks.glance`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#client/acme/overview`; ✎ → Add a widget | "Filters" is offered (Echo's own), its preview showing a control per setting |
| 2 | Add "Filters"; Done | A "Filters" widget with two controls: "Thread" (All threads, then Acme's open threads only) and "Period" (week, month; week chosen). No "Client" control: the page sets it |
| 3 | In "Thread" choose Acme rollout | "Roles to fill" says "thread acme-rollout", "Open positions" shows 3, "Done this week" names Acme rollout; "Threads at a glance" (no thread setting) is unchanged |
| 4 | In "Period" choose month | "Threads at a glance" and "Done this week" both count this month |
| 5 | ⚙ on "Done this week": set Period to week; Done | Filters' "Period" says "Mixed" (the widgets differ); choosing week again makes it one value |
| 6 | Reload the page | The filters and the widgets are as left (they are the widgets' settings, kept with the page) |
| 7 | `target/debug/echoctl today --client acme filter` | Lists thread (acme-rollout, 3 widgets) and period (week, 2 widgets) |
| 8 | `target/debug/echoctl today --client acme filter thread ""`; wait about 10 seconds | Every widget shows all of Acme's threads again; Filters' "Thread" says All threads |
| 9 | `target/debug/echoctl today --client acme filter period year` | Refused: not a value it takes (week, month); nothing changed |
| 10 | `target/debug/echoctl today --client acme filter client globex` | Refused: the page sets client |
| 11 | Open `{{base_url}}/` (Today); ✎ → Add "Filters" | Offered on Today too; there "Client" is a control as well (Today sets nothing) |
| 12 | Remove "Filters" from Acme's Overview | The widgets keep the settings it set |

## Expected Final State
Acme's Overview widgets keep the settings last chosen; Filters is gone from it; Today holds Filters.

## Teardown
- Remove Filters from Today; `target/debug/echoctl today --client acme reset`

## Changes
- Changes Today's and Acme's dashboard layouts in the test config.
