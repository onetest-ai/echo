---
id: TC-052
title: "A plugin's table on a dashboard never traps the wheel: 20 rows at height 1 draw 8 and \"+12 more\", the wheel over it scrolls the page; +12 more shows all in place and Show less folds back; at height 2 it draws 16, and its provider is told so"
priority: high
type: functional
requirements: [F-TODAY-32]
module: today
run: agent
platform: any
data: standard
tags: [today, dashboards, widgets, plugins, scroll]
---

# TC-052: A widget never scrolls inside the page

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; `{{base_url}}/` loaded once after the steps below; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`); the browser 1440 × 800
- The test marketplace with "board-demo" (its "Team" table: 20 people; its note says how many rows it was told are drawn): `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install board-demo --from test-plugins --yes`
- A client dashboard holding it and two more: `$E dashboard add Team --client`; `$E today --client acme --board team add plugin:board-demo:team --size L`; `$E today --client acme --board team add plugin:board-demo:glance --size M`; `$E today --client acme --board team add plugin:board-demo:people --size M`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#client/acme/b-team` (reload once if a widget hasn't answered yet) | "TEAM · ACME 20 · drawn: 8 rows at height 1"; "Person 1" to "Person 8" with their roles; under them "+12 more"; no scrollbar inside the table |
| 2 | Put the pointer over "Person 4" and turn the wheel down | The page scrolls (to its end: Team glance and People come into view); the table's rows don't move inside it |
| 3 | Click "+12 more" | All 20 rows, to "Person 20", in place; the button says "Show less" and has the focus; the page grew with them |
| 4 | Back at the top, the pointer over "Person 5", turn the wheel down | The page scrolls down past the table; nothing scrolls inside it |
| 5 | Click "Show less" | 8 rows and "+12 more" again |
| 6 | `$E today --client acme --board team height plugin:board-demo:team 2`; reload | "drawn: 16 rows at height 2"; "Person 1" to "Person 16" and "+4 more" |
| 7 | Teardown below | — |

## Teardown
- `$E dashboard remove team --client`; `$E plugin remove board-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The counts by size and height (tables 8 a row of height; lists 3 / 5 / 10; bars 4 / 6 / 12), the fold and the grouped table's counts: `today-plugin-widgets.test.mjs`; what the provider is told and its re-ask on a new height: `jshost::tests` (the ci plugin's `builds`) and `today_layout::tests::a_widget_draws_and_is_told_the_rows_items_and_bars_its_size_and_height_hold`.
- The same on Today (a fill table): TC-051; a hug table with groups: TC-042.
