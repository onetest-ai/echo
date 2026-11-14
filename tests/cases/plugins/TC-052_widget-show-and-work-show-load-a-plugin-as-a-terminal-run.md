---
id: TC-052
title: echoctl widget show and echoctl work show load a plugin as a terminal run (ctx.terminal true), so its start-up work doesn't run from a read-only command
priority: medium
type: functional
requirements: [F-PLUG-16]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, cli, terminal]
---

# TC-052: A read-only CLI command doesn't start a plugin's start-up work

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake` (port 481xx)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`; `echoctl` is `target/debug/echoctl`
- One made-up plugin **Acme startup** in a local marketplace `acme-local`: its start-up writes `terminal.txt` (what `ctx.terminal` was) and, only when it is not a terminal run, `started.txt`; it gives a widget `w` and work rows:

```bash
M="$D/market"; mkdir -p "$M/plugins/acme-startup"
echo '{"name":"acme-local","plugins":[{"id":"acme-startup","source":"./plugins/acme-startup"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-startup","name":"Acme startup","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","widgets":[{"id":"w","label":"Acme","kind":"stat"}],"uses":{"work":{}}}' > "$M/plugins/acme-startup/echo-plugin.json"
cat > "$M/plugins/acme-startup/main.js" <<'JS'
export function apply(ctx) {
  ctx.store.write('terminal.txt', String(ctx.terminal))
  if (!ctx.terminal) ctx.store.write('started.txt', 'ran')
  ctx.widgets.provide('w', () => ({ value: 1 }))
  ctx.work.provide(() => [])
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin), the plugin's store

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-startup --yes`; `S="$ECHOCTL_HOME/.echo/plugins/acme-startup"; rm -f "$S/started.txt" "$S/terminal.txt"` | Installs |
| 2 | `echoctl widget show plugin:acme-startup:w`; `cat "$S/terminal.txt"; ls "$S"` | The widget's data (value 1); `true`; the listing has no `started.txt`: the plugin's start-up work didn't run |
| 3 | `rm -f "$S/terminal.txt"`; `echoctl work show`; `cat "$S/terminal.txt"; ls "$S"` | The work (JSON, no rows); `true`; still no `started.txt` |
| 4 | `rm -f "$S/terminal.txt"`; `echoctl plugin call acme-startup nothing` (it fails: no such command); `cat "$S/terminal.txt"` | `true`: `plugin call` loads it the same way |

## Expected Final State
Acme startup saw `ctx.terminal` true from every command, and its start-up work never ran.

## Teardown
- `echoctl plugin remove acme-startup --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the issue (#625): every CLI command that loads a plugin only to answer is a terminal run. The Rust test `cli_plugin_terminal::widget_show_and_work_show_load_a_plugin_as_a_terminal_run` checks the same on the binary.
