---
id: TC-044
title: "Removing a marketplace removes the plugins installed from it — asked first, in the window (keep or delete their data) and in the CLI (--yes, --delete-data); their prefix and namespace go with them; a marketplace with nothing installed is removed as before"
priority: high
type: functional
requirements: [F-PLUG-07, F-PLUG-10]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, marketplaces, cli]
---

# TC-044: Removing a marketplace removes its plugins

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`); the browser 1440 × 900
- A made-up thread: `$E project create "Alpha"` (id `alpha`)
- Two copies of the test marketplace under other names, each in the run's scratch folder: `for n in one two; do M="$(dirname "$ECHOCTL_HOME")/market-$n"; cp -R "$PWD/tests/fixtures/plugins" "$M"; done` (each offers share-a, share-b, table-demo, …)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$(dirname "$ECHOCTL_HOME")/market-one" --name one`; `$E plugin install share-a --from one --yes`; `$E plugin install share-b --from one --yes`; `$E plugin call share-a sset '{"key":"test-plugins.watch","value":["ACME-1"]}'`; `$E plugin call share-a tset '{"thread":"alpha","key":"codes","value":"mine"}'` (share-a's own data); `ls "$ECHOCTL_HOME/.echo/plugins"` | share-a and share-b installed; a data folder `share-a` there |
| 2 | `$E plugin marketplace remove one` | Lists the plugins installed from it — "Share A (share-a)", "Share B (share-b)" — says they are removed with it, then refused: "run again with --yes …"; `$E plugin list` still has both, and the marketplace `one` |
| 3 | `$E plugin marketplace remove one --yes` | "Removed one and the plugins installed from it: share-a, share-b"; `$E plugin list` has neither and no marketplace `one`; share-a's data folder still there (kept without `--delete-data`). Then `$E plugin marketplace add "$(dirname "$ECHOCTL_HOME")/market-two" --name two`; `$E plugin install share-a --from two --yes`: `$E plugin call share-a tget '{"thread":"alpha","key":"codes"}'` → `"mine"` (its data kept); `sget` of `test-plugins.watch` → `["ACME-1"]` (shared values stay); `sset` of it to `[]` passes — the namespace isn't held by the removed source any more |
| 4 | `$E plugin install table-demo --from two --yes`; load `{{base_url}}/#settings/plugins`, tab "Marketplaces"; ⋯ on "two" | The menu's item reads "Remove…" (not "installed plugins stay") |
| 5 | Click "Remove…" | Under "two" a confirmation: "Remove two? Its 2 installed plugins go with it: Share A, Table demo." with "Cancel", "Remove, keep their data" and "Remove with their data" |
| 6 | "Cancel" | The confirmation goes; nothing removed (`$E plugin list` has share-a and table-demo) |
| 7 | "Remove…" again, then "Remove with their data" | "two" leaves the list; the Installed tab has neither plugin; `ls "$ECHOCTL_HOME/.echo/plugins"` has no `share-a` folder (its data deleted) |
| 8 | `$E plugin marketplace add "$(dirname "$ECHOCTL_HOME")/market-one" --name one`; `$E plugin marketplace remove one` (nothing installed from it) | Removed at once, no `--yes` asked: "Removed one" |
| 9 | `$E plugin marketplace remove onetest`; `$E plugin marketplace remove my-plugins` | Refused as before: the built-in one can be hidden, not removed; My plugins is always there |

## Teardown
- `$E project delete alpha`; `rm -rf "$(dirname "$ECHOCTL_HOME")"/market-one "$(dirname "$ECHOCTL_HOME")"/market-two`; `tests/fixtures/serve.sh down <dir>`; close the browser tab

## Notes
- Each plugin is removed as `echoctl plugin remove` does (its laid-out folder for Claude, its scripts, its sign-ins, its widgets), before the marketplace goes; then the skills prefixes and shared namespaces no installed plugin of a source holds are released (`prefixes.json`, `namespaces.json`): the Rust test `plugins::tests::removing_a_marketplace_removes_its_plugins_and_releases_what_they_held`.
