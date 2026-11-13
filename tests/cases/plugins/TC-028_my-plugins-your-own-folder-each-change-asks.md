---
id: TC-028
title: My plugins — your own plugins in the Echo data folder: listed as a built-in marketplace, each install and change asks with the whole sheet, a change to its files is its update, a copy installed, never mixed with another marketplace's; the build skill's commands (plugin mine, plugin guide, widget show, today add on a dashboard)
priority: high
type: functional
requirements: [F-PLUG-26, F-PLUG-27]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, widgets, self-service, security]
---

# TC-028: My plugins

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env` (`E=target/debug/echoctl`); the browser 1440 × 1000

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin mine` | JSON: `folder` the data folder's `plugins` (made now), `plugins` [] |
| 2 | Open `{{base_url}}/#settings/plugins`, the Marketplaces tab | "My plugins" right after the built-in one, tagged "built in", its folder as its source, "Your own plugins and widgets, a folder each — ask Claude to make one (the build skill)."; "Read again", no ⋯ menu |
| 3 | `$E plugin marketplace remove mine`; `$E plugin marketplace hide mine` | Both refused: "My plugins is always there…", "My plugins can't be hidden…" |
| 4 | In the folder make `team-pulse/echo-plugin.json` `{"id":"team-pulse","name":"Team pulse","version":"0.1.0","summary":"A test","main":"main.js","widgets":[{"id":"open","label":"Open items","kind":"stat","sizes":["S","M"],"params":[{"key":"thread","label":"Thread","type":"thread"}]}]}` and `team-pulse/main.js` `export function apply(ctx) { ctx.widgets.provide('open', ({ params }) => ({ value: 3, label: 'open in ' + (params.thread \|\| 'all') })) }`; `$E plugin mine` | `plugins` [{"id": "team-pulse", "version": "0.1.0", "error": null}] |
| 5 | `$E plugin install team-pulse --from mine` | The sheet printed, then refused: "run again with --yes to agree" (nothing installed: `$E plugin list` has no team-pulse) |
| 6 | `$E plugin install team-pulse --from mine --yes`; `$E widget show plugin:team-pulse:open` | "Installed Team pulse 0.1.0"; the widget's data `{value: 3, label: "open in all"}` |
| 7 | `$E today --thread acme-rollout add plugin:team-pulse:open --size S`; open `{{base_url}}/#project/acme-rollout/brief` | Over "What it is": "Open items" with 3, "open in acme-rollout" |
| 8 | Change `main.js`'s value to 5 (same version); `$E plugin update team-pulse` | The sheet printed and refused without `--yes`; `$E plugin update team-pulse --yes` updates it (0.1.0 again); `$E widget show plugin:team-pulse:open` says 5 |
| 9 | `$E plugin guide` | Echo's plugin authoring rules (Markdown: Manifest, Host half, Client half, Publishing) |
| 10 | `ECHOCTL_HEADLESS=1 $E plugin install team-pulse --from mine --yes` | Refused: "plugins are installed and updated by you, not from inside a run Echo started" |
| 11 | `$E today --thread acme-rollout reset`; `$E plugin remove team-pulse` | The folder `team-pulse` still in My plugins (yours to delete) |

## Teardown
- Delete the `team-pulse` folder; `tests/fixtures/serve.sh down <dir>`; close the browser tab
