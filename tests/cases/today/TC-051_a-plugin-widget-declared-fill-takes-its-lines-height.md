---
id: TC-051
title: A plugin's widget declared height "fill" is as tall as its line (the rest of the view), like Your day and My work, its rows scrolling inside the card under its header; the widget beside it takes the line's height, a hug widget under it its own
priority: medium
type: functional
requirements: [F-TODAY-19, F-TODAY-12]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, layout]
---

# TC-051: A plugin widget declared "fill" takes its line's height

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 900
- The test plugins `fill-demo` and `widget-demo` from `tests/fixtures/plugins`: `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install fill-demo --yes`; `target/debug/echoctl plugin install widget-demo --yes`
- Today holding nothing: `target/debug/echoctl today remove <widget>` for each widget `target/debug/echoctl today show` lists

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl widget list` | `plugin:fill-demo:rows` (kind "table") with `"height": "fill"`; `plugin:widget-demo:open` and `plugin:widget-demo:sprint` with `"height": "hug"` |
| 2 | `target/debug/echoctl today add plugin:fill-demo:rows --size L`; open `{{base_url}}/#today` (reload once if it hasn't answered yet) | "LONG TABLE 60", the width of Today, its card reaching down to about 24 px above the status bar (the rest of the view, not the 60% of the window a hug table stops at); the page has no scrollbar; "Row 1" at the top, "Row 60" not in view |
| 3 | Scroll with the wheel over the table's rows to their end | The rows scroll inside the card to "Row 60"; the header "LONG TABLE 60" and the column names NAME and COUNT stay at the card's top; the page itself doesn't move |
| 4 | Resize the browser to 1440 × 1100; reload | The card grows with the window: its bottom again about 24 px above the status bar |
| 5 | Back to 1440 × 900. `target/debug/echoctl today add plugin:widget-demo:open --size S --after plugin:fill-demo:rows`; `target/debug/echoctl today add plugin:widget-demo:sprint --size M`; reload | Long table (narrowed to two columns) and "OPEN TICKETS" (one column) on one line, both cards ending at the same height; "SPRINT" under them, as tall as its three numbers (a hug widget), the bottom of the view; Long table's rows still scroll inside; the page has no scrollbar |
| 5a | `target/debug/echoctl today --client acme add plugin:fill-demo:rows --size L`; open `{{base_url}}/#client/acme/overview` (reload once if its placeholder stays: #598) | On a client's Overview (a dashboard: its lines as tall as their widgets) the table keeps its cap: its card stops at about 60% of the window and its rows scroll inside it, not 60 rows down the page. `target/debug/echoctl today --client acme remove plugin:fill-demo:rows` |
| 6 | `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove fill-demo`; `target/debug/echoctl plugin remove widget-demo` | Today as it comes |

## Teardown
- `target/debug/echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`; close the browser tab

## Notes
- `fill-demo`'s table has a client setting only so a client's Overview can take it (step 5a).
- That a plugin widget's data carries its declared height is in the Rust test `widgets::a_plugin_widgets_data_carries_its_declared_height`.
- Found as #523: Today read only its three core fill widgets, so a plugin's `fill` widget hugged its content (a table stopped at about 60% of the window, TC-042).
