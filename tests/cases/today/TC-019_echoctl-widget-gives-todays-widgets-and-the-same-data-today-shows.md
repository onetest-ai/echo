---
id: TC-019
title: echoctl widget list gives the widgets Today can show, and widget show a widget's data — the same Today gets; an unknown one names widget list
priority: medium
type: functional
requirements: [F-TODAY-10]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, cli]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-019: echoctl widget gives Today's widgets and the same data Today shows

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); CLI steps run after `. <dir>/env`; `target/debug/echoctl` is the debug build of this checkout

## Test Data

| Widget | Kind | Temporal | Heights |
|---|---|---|---|
| `core:calendar.agenda`, `core:calendar.calendar` | day | no | 1, 2 |
| `core:calendar.strip` | day | no | 1 |
| `core:calendar.choices` | choices | yes | 1, 2 |
| `core:calendar.next` | next | yes | 1, 2 |
| `core:tasks.glance` | glance | no | 1, 2 |
| `core:tasks.done` | glance | no | 1 |
| `core:calls.week` | calls | no | 1 |
| `core:tasks.mine` | work | no | 1, 2 |
| `core:needs.now` | needs | yes | 1, 2 |
| `core:routines.cards` | cards | yes | 1, 2 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl widget list` | JSON: the eleven widgets of Test Data, in that order (a plugin's and a routine's card widgets after them when there are some), each with `part`, `label`, `about`, `kind`, `sizes` (of S, M, L), `height` (hug or fill), `heights` (rows high it may take) and `temporal` as in the table, and `params` (its settings: today/TC-032) |
| 2 | In the page: `fetch('/api/widgets').then(r => r.json())` | The same eleven ids, in the same order |
| 3 | `target/debug/echoctl widget show core:tasks.mine` and, in the page, `fetch('/api/today').then(r => r.json())` | The CLI's `mine.items` are the page's `work.mine.items`: the same titles, in the same order (My work's "I owe" cards) |
| 4 | `target/debug/echoctl widget show core:needs.now` | `items` as the page's `needs.items` (the same ids), and `"empty": false` (the data has items in the last 24 h) |
| 5 | `target/debug/echoctl widget show core:nope` | Exit code 1, "no widget \"core:nope\": `echoctl widget list` names them" |

## Expected Final State
Nothing changed: the commands only read.

## Notes
- Phase 1 of the Today widgets (#315, #316): the catalog has Echo's widgets for what Today shows now; the rest of the agreed set and plugins' widgets come with their phases.
