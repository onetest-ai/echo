---
id: TC-033
title: A plugin's own page as a widget (kind page) — drawn by its window half (ctx.widgets.draw) inside Echo's frame, from the data its host half gives, with the widget's size and theme; its text escaped by the plugin; gone with the plugin
priority: medium
type: functional
requirements: [F-TODAY-24]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, untrusted-text]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-033: A plugin's own page as a widget

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 900
- The test plugin installed as in today/TC-029 step 1 (`widget-demo` 0.4.0: it has a window half)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today add plugin:widget-demo:board --size M --above core:calendar.agenda`; open `{{base_url}}/#today` (reload once if it hasn't answered yet) | "Sprint board" in Echo's frame (its label on top): three boxes, "5 To do", "2 Doing", "9 <i>Done</i>" — the `<i>` shown as text, nothing italic; its box says the size (`data-size` M) and the theme (light or dark) |
| 2 | `target/debug/echoctl today size plugin:widget-demo:board L`; reload | The page drawn again at L (`data-size` L) |
| 3 | `target/debug/echoctl plugin disable widget-demo`; reload | Sprint board isn't drawn (its plugin is off); `echoctl today` still lists it (it keeps its place) |
| 4 | `target/debug/echoctl plugin enable widget-demo`; reload | It is back |
| 5 | `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove widget-demo` | Today as it comes |

## Notes
- A page widget needs a window half: the manifest check is the Rust test `jshost::a_manifests_widgets_are_checked`.
