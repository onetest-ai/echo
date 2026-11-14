---
id: TC-029
title: A plugin installed, updated or switched off while the window is open reaches it without a restart — its widgets in Add a widget, its client half once in place of the old, its host half answering
priority: high
type: functional
requirements: [F-PLUG-28]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, widgets, window]
---

# TC-029: A plugin changed while the window is open

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- A local plugin "mine-demo" in My plugins: `target/debug/echoctl plugin mine`, then a copy of `tests/fixtures/plugins/plugins/widget-demo` in `<the folder it prints>/mine-demo` with `id` "mine-demo", `name` "Mine demo", `version` "0.1.0", and two lines added at the top of `apply(ctx)` in its `client.js`: `window.__applies = (window.__applies || 0) + 1` and `ctx.listen('mine-ping', () => { window.__pings = (window.__pings || 0) + 1 })`; `target/debug/echoctl plugin mine` again (it reads the folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Today → Edit → Add a widget | No part "Mine demo" |
| 2 | Leave the window as it is (no reload, editing open); `target/debug/echoctl plugin install mine-demo --yes`; wait a few seconds | Add a widget shows part "Mine demo" with its eight widgets; `window.__applies` is 1; `document.dispatchEvent(new Event('mine-ping'))` makes `window.__pings` 1; the "Open tickets" preview shows its data (the host half answered), not "Couldn't get its data" |
| 3 | In the copy: `version` "0.1.1", a ninth widget "extra" (a copy of "note" labelled "Extra note"); `target/debug/echoctl plugin mine`; `target/debug/echoctl plugin update mine-demo --yes`; wait a few seconds; set `window.__pings = 0`, dispatch `mine-ping` once | Add a widget lists "Extra note" (nine widgets); `window.__applies` is 2 (the new half ran once); `window.__pings` is 1 (the old half's listener is gone) |
| 4 | `target/debug/echoctl plugin disable mine-demo`; wait a few seconds; dispatch `mine-ping` | No part "Mine demo" in Add a widget; no ping counted |
| 5 | Done; `target/debug/echoctl plugin remove mine-demo` | Today as before |

## Notes
- The same holds for a change made in Settings → Plugins (Install, Update, the switch, Remove), which the window already followed; this case checks the change made outside the window.
