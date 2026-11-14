---
id: TC-041
title: "`echoctl plugin install <folder>` copies the plugin into My plugins and installs it from there with the sheet; the same folder again is its update; a My plugins folder of that id that came from elsewhere is never overwritten"
priority: high
type: functional
requirements: [F-PLUG-26, F-PLUG-10]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, cli, self-service]
---

# TC-041: Install a plugin from a folder

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps after `. <dir>/env` (`E=target/debug/echoctl`)
- A made-up plugin outside the data folder, in the run's scratch folder: `S="$(dirname "$ECHOCTL_HOME")/src/folder-pulse"; mkdir -p "$S"`; `echo '{"id":"folder-pulse","name":"Folder pulse","version":"0.1.0","summary":"A test","main":"main.js","widgets":[{"id":"open","label":"Open items","kind":"stat","sizes":["S","M"]}]}' > "$S/echo-plugin.json"`; `echo "export function apply(ctx) { ctx.widgets.provide('open', () => ({ value: 3, label: 'open' })) }" > "$S/main.js"`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install "$S"` | The sheet printed, then refused: "run again with --yes to agree"; nothing installed (`$E plugin list` has no folder-pulse) |
| 2 | `$E plugin install "$S" --yes`; `$E plugin mine`; `$E widget show plugin:folder-pulse:open` | Prints that it copied the folder into My plugins (naming My plugins' `folder-pulse`), then "Installed Folder pulse 0.1.0"; `plugin mine` lists `folder-pulse` 0.1.0; the widget's data `{value: 3, label: "open"}`; `$E plugin list` has it installed from `my-plugins` |
| 3 | Change `$S/main.js`'s value to 5 (same version); `$E plugin install "$S"` | Copied again, then installed again without `--yes` (nothing new asked); `$E widget show plugin:folder-pulse:open` says 5 |
| 4 | Add `"uses":{"network":{"hosts":["api.example.com"]}}` to `$S/echo-plugin.json`; `$E plugin install "$S"` | The sheet printed with the new network line (new), refused without `--yes`; the installed plugin is still the one before (`widget show` says 5) |
| 5 | `cd "$S" && $E plugin install . --yes; cd -` | `.` is the folder: copied and installed with the network line |
| 6 | A second, different folder with the same id: `T="$(dirname "$ECHOCTL_HOME")/src/other"; mkdir -p "$T"; cp "$S/echo-plugin.json" "$S/main.js" "$T/"`; `$E plugin install "$T" --yes` | Refused: My plugins already has a folder `folder-pulse` that wasn't copied from that folder (its path named); My plugins' copy and the installed plugin unchanged |
| 7 | `$E plugin install "$(dirname "$ECHOCTL_HOME")/src/nothing-here"` | Refused: not a folder (or no plugin in it) — nothing copied |
| 8 | `ECHOCTL_HEADLESS=1 $E plugin install "$S" --yes` | Refused: "plugins are installed and updated by you, not from inside a run Echo started"; nothing copied |
| 9 | `$E plugin install folder-pulse --from my-plugins --yes` | An id still installs as before (from My plugins' copy) |
| 10 | `$E plugin remove folder-pulse` | Removed; the folder `folder-pulse` stays in My plugins (yours to delete), and `$S` untouched |

## Teardown
- Delete My plugins' `folder-pulse` folder and `$S`, `$T`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Where a copy came from is kept in My plugins' `.sources.json` (a file, not a plugin folder: My plugins doesn't list it). A folder in My plugins that you made yourself has no entry there, so a folder install never replaces it.
- With no data folder chosen, `install <folder>` is refused like `plugin mine` (My plugins needs one) — the Rust test covers it.
