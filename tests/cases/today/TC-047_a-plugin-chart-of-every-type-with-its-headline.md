---
id: TC-047
title: A plugin's chart of every type — line, area, bar, stacked, stacked-bars, bars, pie, donut, funnel — with its headline (value, label, change with an arrow and a tone), Echo's palette and a legend for 2+, every mark titled; stats with a trend; data it can't draw says why
priority: high
type: functional
requirements: [F-TODAY-22]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, charts]
---

# TC-047: A plugin's chart of every type with its headline

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- The test plugin `chart-demo` from `tests/fixtures/plugins` (marketplace "test-plugins"): `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install chart-demo --yes`
- Today emptied of Echo's widgets (`target/debug/echoctl today remove <widget>` for each of `echoctl today`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `today add plugin:chart-demo:calls --size M`; `today add plugin:chart-demo:hours --size S --after plugin:chart-demo:calls`; `today add plugin:chart-demo:deals --size M`; `today add plugin:chart-demo:staffing --size M --after plugin:chart-demo:deals`; open `{{base_url}}/#today` (reload once if one hasn't answered) | Calls by week: under its header "24 recorded ↑ 2 vs last week" (the change green), two lines (violet, magenta), under them Oct 6 … Nov 10 (dates as Echo writes them) and a legend "recorded · with minutes". Billable hours (S): "24 h this week ↑ 2 h vs last week", a violet area, no legend. Deals by quarter: grouped bars won/lost by Q1–Q4. Staffing by week: stacked open/proposed/booked with a gap between segments |
| 2 | Hover a point of Calls by week, a bar of Deals, a segment of Staffing | Each says its value: "Nov 10 · recorded: 24", "Q4 · won: 24", "Nov 10 · booked: 9" |
| 3 | `today add plugin:chart-demo:status --size M`; `today add plugin:chart-demo:tasks --size S --after plugin:chart-demo:status`; `today add plugin:chart-demo:time --size M`; `today add plugin:chart-demo:positions --size M --after plugin:chart-demo:time`; reload | Project status: "38% on track, across 3 projects ↑ 4 vs last week" (the change red: not good), three rows of 100% stacks in green / amber / red (the series' tones), 60%, 35%, 20% at their ends. Open tasks by thread: "17 open tasks", four violet bars with 7, 5, 3, 2. Time by kind: a pie and its legend Delivery 42% … Travel 3%, Other 2% (Admin under 3% folded). Positions: a donut with "16 positions" in its hole and Booked 9, Proposed 4, Open 3 |
| 4 | `today add plugin:chart-demo:pipeline --size M`; `today add plugin:chart-demo:week --size M --after plugin:chart-demo:pipeline`; `today add plugin:chart-demo:broken --size M`; reload | Pipeline: "9 won ↑ 2 vs last month", Leads 120 … Won 9, each stage after the first with its share of the one before (53%, 47%, 47%, 64%). This week: 23, 19, 7 with their labels, each with a line across its column ending in a dot. Broken chart: "Couldn't draw this — a pie takes no negative values (only line and bar do)" |
| 5 | `today size plugin:chart-demo:calls S`; reload | Calls by week at S: its headline and lines, no legend |
| 5a | `today size plugin:chart-demo:time L`; `today size plugin:chart-demo:positions L`; reload | A wide pie or donut grows (its disc up to about 190 px) and its legend becomes one aligned grid — swatch, name, a share bar across the free width, the value; at M (`today size … M`) the legend is compact again, no share bars. This week beside a taller card: its trend lines grow to fill the card's height (up to 140 px), each still ending in a dot |
| 6 | Switch the window to the dark theme (Settings → Appearance, or the system's) | The same charts in the dark palette (lighter violet, magenta, blue), text in the theme's ink; the legend swatches match the lines |
| 7 | `target/debug/echoctl today reset` | Today as it comes |

## Teardown
- `target/debug/echoctl plugin remove chart-demo`; `target/debug/echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`; close the browser tab
