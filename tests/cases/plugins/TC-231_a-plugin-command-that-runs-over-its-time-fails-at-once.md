---
id: TC-231
title: A plugin command whose JavaScript runs over 5 s without an await fails at once with "took too long", echoctl plugin call exits 1, and the plugin answers its next call
priority: high
type: functional
requirements: [F-PLUG-138]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, cli, limits]
---

# TC-231: A plugin command that runs over its time fails at once

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run: `tests/fixtures/serve.sh up <dir> <port>`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- A plugin folder `<dir>/spinner`: `echo-plugin.json` `{"id": "spinner", "name": "Acme spinner", "version": "0.1.0", "main": "main.js"}` and `main.js` `export function apply(ctx) { ctx.command('spin', () => { while (true) {} }); ctx.command('ok', () => 1) }`; installed: `$E plugin install <dir>/spinner --yes`

**Changes:** the installed plugin

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `time $E plugin call spinner spin; echo "exit=$?"` | Within about 6 s: `Error: spinner's job took too long: Echo stops a plugin's JavaScript after 5 s`, `exit=1`; the process has ended (no `echoctl` left running at full CPU) |
| 2 | `$E plugin call spinner ok` | `1` |

## Expected Final State
The plugin is installed and answers.

## Teardown
- `$E plugin remove spinner --delete-data`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Before #817 step 1 never returned: the interrupted job's promise never settled, so the call waited out its hour.
