---
id: TC-029
title: A plugin's widgets on Today — declared in its manifest (the install sheet says how many), in the catalog under its name, put on with echoctl today or Add a widget, their data from the plugin's provide (asked by Echo) or set / clear, drawn by kind (stat, stats, list, status, card) with its text escaped and links only to echo://; data Echo can't draw says why
priority: high
type: functional
requirements: [F-TODAY-19]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, untrusted-text]
---

# TC-029: A plugin's widgets on Today

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- The test plugin `tests/fixtures/plugins` (marketplace "test-plugins", plugin "widget-demo": one widget of each kind, and one whose data is wrong)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install widget-demo --yes` | The sheet lists "Runs its own code in Echo, with your rights" and "Adds 7 widgets you can put on Today"; "Installed Widget demo 0.3.0" |
| 2 | `target/debug/echoctl widget list`; `target/debug/echoctl widget show plugin:widget-demo:builds` | Seven `plugin:widget-demo:…` widgets (open, sprint, trend, builds, ci, note, broken), part "Widget demo", each with its kind and sizes; `widget show` asks the plugin and waits: `kind` "list", `data.items` with 3 items, `empty` false |
| 3 | `today add plugin:widget-demo:open --size S --above core:needs.now`, the same for `ci` (S) and `broken` (S) after it, `sprint` (M) and `builds` (M) on a row above Needs, `note` (M) on its own; `target/debug/echoctl plugin call widget-demo ci '{"state":"warn"}'` and `… call widget-demo note`; open `{{base_url}}/#today` (reload once if a widget hasn't answered yet) | Open tickets: "12" amber, "open tickets"; CI: an amber dot, "CI main is green"; Broken: "Couldn't draw this — a stat needs value"; Sprint: 21 done (green), 8 to do, 2 blocked (red); Last builds: three lines — "main #41 passed / 2 min ago", "Open the task" as a link, and `<img src=x onerror=alert(1)> kept as text` as text (no image, no dialog, no link to evil.example); Note: "Release notes", "Ship Friday." and "Tell support." on two lines |
| 4 | Click "Open the task" | Echo opens echo://task/acme-rollout/T001 (the task's place, or says why it can't) |
| 5 | `target/debug/echoctl plugin call widget-demo clear-note`; reload | Note is gone from Today (it has nothing to show); `echoctl widget show plugin:widget-demo:note` says `"empty": true` |
| 6 | Edit Today → Add a widget, part "Widget demo" | Its seven widgets listed with their about lines, the ones on Today "on Today" |
| 7 | `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove widget-demo` | Today as it comes; `widget list` has no `plugin:widget-demo:` widget |

## Notes
- The drawing of each kind is the node test `today-plugin-widgets.test.mjs`; the manifest's checks and provide / set / clear are the Rust tests in `jshost`.
