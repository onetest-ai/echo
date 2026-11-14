---
id: TC-038
title: An `echoctl` a plugin's script starts through the plugin's `scripts/echoctl` is the Echo that ran the script, on its data and config folders — on test data, the test data, never the real data folder
priority: high
type: functional
requirements: [F-PLUG-25]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, scripts, test-safety]
---

# TC-038: A plugin script's echoctl works on the data Echo runs on

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data: `python3 tests/fixtures/make_data.py <dir> --set standard` (an empty `<dir>`). CLI steps run in Bash after `. <dir>/env`, with `D="$(dirname "$ECHOCTL_HOME")"`. `echoctl` is `target/debug/echoctl`, built right before the run. Before going on, confirm that `ECHOCTL_HOME` is inside `<dir>`. No window step.
- A made-up plugin **Acme CRM** in a local marketplace. Its script runs in its skill's session copy, so `../../scripts/echoctl` is the plugin's launcher. The script refuses to start `echoctl` unless a test data folder was named, so a regression fails the case instead of reading or changing the real data folder:

```bash
M="$D/market"; K="$M/plugins/acme-crm"
mkdir -p "$K/skills/crm/scripts"
echo '{"name":"acme-local","plugins":[{"id":"acme-crm","source":"./plugins/acme-crm"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-crm","name":"Acme CRM","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","skills":["skills/crm"],"agent":{"prefix":"acme"},"uses":{"scripts":[{"skill":"crm","script":"scripts/clients.sh"}]}}' > "$K/echo-plugin.json"
printf -- '---\nname: crm\ndescription: Keeps Acme CRM clients in Echo.\n---\n\nRun scripts/clients.sh.\n' > "$K/skills/crm/SKILL.md"
printf '#!/bin/sh\ncase "${ECHOCTL_HOME:-}" in */home) ;; *) echo "no test data folder: not running echoctl"; exit 3;; esac\necho "home=$ECHOCTL_HOME config=$ECHOCTL_CONFIG_DIR bin=$ECHOCTL_BIN"\n../../scripts/echoctl client add Hooli --domains hooli.example >/dev/null && ../../scripts/echoctl client list\n' > "$K/skills/crm/scripts/clients.sh"
echo "export function apply(ctx) { ctx.command('clients', () => ctx.scripts.run('crm', 'scripts/clients.sh', [])) }" > "$K/main.js"
echoctl plugin marketplace add "$M" --name acme-local
```

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-crm --yes` | "Installed Acme CRM 0.1.0" |
| 2 | `echoctl plugin call acme-crm clients` | `"status": 0`. The first line of `stdout` is `home=` followed by `$ECHOCTL_HOME`, then `config=` followed by `$ECHOCTL_CONFIG_DIR`, then `bin=` followed by the full path of `target/debug/echoctl`. After it comes the client list of the test data (Initech, Acme, Globex, Umbrella, …), which ends with Hooli (`hooli.example`). |
| 3 | `echoctl client list` | The test data's clients include Hooli (`"id": "hooli"`): the script's `echoctl` changed the data Echo runs on |

## Expected Final State
Acme CRM is installed, and the test data has the client Hooli. The real data folder and config are untouched.

## Teardown
- `echoctl client remove hooli`; `echoctl plugin remove acme-crm --delete-data`; `echoctl plugin marketplace remove acme-local`; `rm -r "$M"`

## Notes
- Before #569 a script got no `ECHOCTL_HOME`, `ECHOCTL_CONFIG_DIR` or `ECHOCTL_BIN`. An `echoctl` it started was then whatever the launcher found (the real config's `app-path`, the installed Echo) and acted on the real data folder. Step 2 then returns `"status": 3` "no test data folder: not running echoctl".
- What Echo sets for a script, and that its secrets stay withheld, is proven by the Rust test `plugins::scripts::an_echoctl_a_script_starts_is_this_echo_on_its_data_and_config_folders_with_its_secrets_still_withheld`. The open guard is `plugins/TC-032`.
