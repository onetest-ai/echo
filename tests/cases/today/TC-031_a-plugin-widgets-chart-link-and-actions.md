---
id: TC-031
title: A plugin's widget as a chart (bars titled by point), with a progress bar, a link (echo:// opens in Echo, https asks before it leaves) and actions (its plugin's commands, run on a click, Today read again)
priority: medium
type: functional
requirements: [F-TODAY-22]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, untrusted-text]
---

# TC-031: A plugin's chart, link and actions

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000
- The test plugin installed as in today/TC-029 step 1 (`widget-demo` 0.3.0)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today add plugin:widget-demo:trend --size M --above core:calendar.agenda`, `today add plugin:widget-demo:ci --size S --after plugin:widget-demo:trend`; `target/debug/echoctl plugin call widget-demo ci '{"state":"bad"}'`; open `{{base_url}}/#today` (reload once if a widget hasn't answered yet) | Open tickets (put it on too: `today add plugin:widget-demo:open --size S --after plugin:widget-demo:ci`): "12 open tickets" and a bar 12 of 20; "Calls a day": five bars (Mon–Fri), the last one stronger, under them "Mon" and "Fri · 4 calls", "Busiest on Tuesday."; hovering a bar says "Tue: 7 calls"; CI: a red dot, "main #42 failed", "Open" and a button "Re-run" |
| 2 | Click CI's "Open" | Nothing leaves Echo yet: under it "Open ci.example in the browser? Click Open again · Cancel" |
| 3 | Click Cancel | The question goes |
| 4 | Click "Re-run" | The plugin's command runs: CI turns green, "main is green", no Re-run |
| 5 | `echoctl widget show plugin:widget-demo:trend` | `kind` "chart", `data.type` "bar", five points |

## Notes
- Each shape (chart types, progress, open, actions, escaping) is the node test `today-plugin-widgets.test.mjs`. Clicking Open a second time opens the browser (not done here).
