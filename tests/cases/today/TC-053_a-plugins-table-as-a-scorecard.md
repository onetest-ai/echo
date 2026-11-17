---
id: TC-053
title: "A plugin's table as a scorecard — a trend in each row, a stage mix bar with its legend, 12 weeks as a heat map under a band, a bold total"
priority: medium
type: functional
requirements: [F-TODAY-33]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, tables]
---

# TC-053: A plugin's table as a scorecard

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; `{{base_url}}/` loaded first; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`); the browser 1440 × 1100
- The test plugin `table-demo` from `tests/fixtures/plugins`: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install table-demo --yes`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E today add plugin:table-demo:scorecard --size L --above core:calendar.agenda`; open `{{base_url}}/#today` (reload once if it hasn't answered yet) | "PIPELINE BY UNIT 3"; over the last twelve heads a band "New deals by week"; heads Unit, Requests, Stage mix, W1 … W12 |
| 2 | Look at Data's row | Requests: a small rising line, its last point a dot, "6" beside it. Stage mix: a thin bar in two colours (about 5 : 3), "9" beside it; hovering it says "Qualified 5, Proposal 3, Lost 1". W1 … W12: 3, ·, 1, 4, 2, ·, 5, 6, 2, 1, ·, 3 — each shaded; W8's 6 (the column's largest) the deepest, the zeros a muted "·" with no shade |
| 3 | Look under the rows and the table | A bold "Total" row: 4, 1, 1, 5, 4, 3, 6, 6, 6, 3, 1, 6 in the week columns (plain numbers, no shade), over a line. Under the table a legend: Qualified, Proposal, Lost (Lost in red) |
| 4 | `$E today size plugin:table-demo:scorecard M`; reload | The same columns (none hidden), the table as wide as the card, sideways only if it doesn't fit; the page scrolls, the table never scrolls up and down inside |
| 5 | Teardown below | — |

## Teardown
- `$E today reset`; `$E plugin remove table-demo`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Each shape, the limits, the refusals and the escaping: `today-plugin-widgets.test.mjs` (a scorecard table).
