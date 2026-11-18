---
id: TC-180
title: A plugin puts a chip of its own on a call's header, with its own popup, and keeps its record per call; a box that throws is left out and the header draws; switched off, its chip leaves every call
priority: medium
type: functional
requirements: [F-PLUG-101, F-PLUG-117]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, calls, client-half, slots]
---

# TC-180: A plugin's chip on a call's header, and its record per call

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`; the browser 1201 × 868
- The test plugins `tests/fixtures/plugins` (marketplace "test-plugins"): "call-demo" (a chip "Filed" / "Not filed" on each call's header with a popup to file the call — Acme's calls in "Acme notes", the rest in "Inbox notes" — and a second header box that always throws) and "menu-demo"

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugins), `<data>/.echo/plugins/call-demo/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install call-demo --yes`; `echoctl plugin install menu-demo --yes` | "Installed Call demo 0.1.0", "Installed Menu demo 0.1.0" |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click its row `[data-call$="-acme-weekly-sync"]` (the first) | The header's buttons read, left to right: "▢ Not filed", "My notes", "What moved…", "Call actions"; one box of Call demo only (`[data-call-slot]` count 1); the console has a warning "call.header: call-demo: Broken: call-demo breaks here on purpose" |
| 3 | Click "▢ Not filed" | A popup under the chip lists "File it in Acme notes" |
| 4 | Click "File it in Acme notes" | The popup closes; the chip reads "▤ Filed" |
| 5 | Click the row of "Globex standup" (`[data-call$="-globex-standup"]`) | Its chip reads "▢ Not filed" (the record is per call); its popup would offer "File it in Inbox notes" |
| 6 | `echoctl plugin call call-demo count` | `1` |
| 7 | Navigate to `{{base_url}}/#settings/plugins`, switch "Call demo on" off (the first such switch); open the call Acme weekly sync again | No chip on its header (`[data-call-slot]` count 0); "My notes" and "Call actions" are there |
| 8 | Switch "Call demo on" on again; open Acme weekly sync | Its chip reads "▤ Filed" (kept in the plugin's store) |

## Expected Final State
Call demo and Menu demo are installed and on; Acme weekly sync is filed in Call demo's store.

## Teardown
- `echoctl plugin remove call-demo --delete-data`; `echoctl plugin remove menu-demo`; `echoctl plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- The box is the plugin's own HTML (Echo's generic `btn` and `menu` classes here); the popup is its own, drawn by its `render` and opened by its own click listener (`ctx.listen`).
- `call` holds only what the page has: `{name, stamp, title, started_at, notes, profile, memo, threads, client}` (`slotCallPage`; `stamp` the call's `YYYY-MM-DD_HHMMSS`, kept when it is retitled); `after` once per box drawn afresh, one that throws left out — `node crates/echoctl/ui/call-slots.test.mjs`.
