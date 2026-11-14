---
id: TC-038
title: A plugin's Claude run (a routine, `ctx.claude.run`) runs as the folder session — in the data folder, with its settings and hooks (the core, the view hook's start, the OneDrive and `.plugins` checks), Echo's and the user's skills, Echo's reads and own edits without a prompt, no read-only guard, the user's one choice to act on its own — its record keeps what it asked for, and `tools: none` keeps no tools
priority: high
type: functional
requirements: [F-PLUG-35]
module: plugins
run: agent
platform: macos
data: standard
ai: fake
tags: [plugins, claude-code, routines, permissions]
---

# TC-038: A plugin's Claude run runs as the folder session

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data with the fake Claude Code, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; CLI steps in Bash after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`; `E=target/debug/echoctl` (inside the repository: Echo finds its own plugin from there), built right before the run; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- **A fake `claude` that writes down how Echo starts it**, in front of the data's fake (never a real account): `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # A run (not --version, not auth) writes its arguments one a line, its folder, its environment and its settings file.
  d="$(dirname "$0")/.."
  case "$1" in --version|auth) ;; *) printf '%s\n' "$@" > "$d/run-args"; pwd > "$d/run-cwd"; env > "$d/run-env"
    prev=; for a in "$@"; do [ "$prev" = --settings ] && cp "$a" "$d/run-settings.json"; prev=$a; done;; esac
  exec "$d/bin/claude.fake" "$@"
  ```
- A made-up plugin **Acme asker** in a local marketplace (only in the run's scratch folder):
  ```bash
  M="$D/market"; mkdir -p "$M/plugins/acme-asker"
  echo '{"name":"acme-local","plugins":[{"id":"acme-asker","source":"./plugins/acme-asker"}]}' > "$M/echo-marketplace.json"
  echo '{"id":"acme-asker","name":"Acme asker","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"claude":{}}}' > "$M/plugins/acme-asker/echo-plugin.json"
  echo "export function apply(ctx) { ctx.command('run', async (a) => { try { await ctx.claude.run({ prompt: 'List what changed', tools: a.tools, dir: 'runs/' + a.dir }); return { ok: true } } catch (e) { return { error: String(e.message || e) } } }) }" > "$M/plugins/acme-asker/main.js"
  $E plugin marketplace add "$M" --name acme-local; $E plugin install acme-asker --yes
  ```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin), its store's `runs/`, `$D/run-*`, `config.json` (step 5)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call acme-asker run '{"tools":"read","dir":"r1"}'` (a routine asking for read-only permissions; the setting "Claude acts on its own" off, as on test data) | An `error` from the data's fake Claude Code (`test data: Claude Code is a fake here`), not a refusal: the run started |
| 2 | `cat "$D/run-cwd"`; `grep -c '^--tools$' "$D/run-args"`; `grep -E '^--(settings\|plugin-dir\|dangerously-skip-permissions\|permission-mode)$' -A1 "$D/run-args"`; `grep -E 'echoctl (status\|project work\|compose\|m365)' "$D/run-args"` | The folder is `$ECHOCTL_HOME` (the data folder); `0` (no tools list of a read-only run); a `--settings` file, `--plugin-dir` of the user's skills (`…/config/my-skills/…`) and of Echo's own plugin; no `--dangerously-skip-permissions`, no `--permission-mode`; among the allowed commands `Bash(… status:*)`, `Bash(… project work:*)` and `Bash(… compose:*)` — Echo's reads and its own data's edits — and nothing of `m365` (no send without a prompt) |
| 3 | `python3 -c "import json,sys; h=json.load(open(sys.argv[1]))['hooks']; c=lambda k: [x['command'].split()[-2:] for e in h.get(k, []) for x in e['hooks']]; print(c('SessionStart'), c('PreToolUse'), c('UserPromptSubmit'))" "$D/run-settings.json"` | `SessionStart` runs `view-hook` and `core --hook`; `PreToolUse` runs `onedrive-hook` and `plugins-hook`; `UserPromptSubmit` empty — what the window shows isn't an unattended run's |
| 4 | `grep -E '^ECHOCTL_(TERMINAL\|SESSION_PROFILE\|RUN_SESSION\|RUN_MODE)=' "$D/run-env"`; `cat "$ECHOCTL_HOME/.echo/plugins/acme-asker/runs/r1/permissions.json"` | `ECHOCTL_TERMINAL=1`, `ECHOCTL_SESSION_PROFILE=<the default profile>`, `ECHOCTL_RUN_SESSION=asks`, `ECHOCTL_RUN_MODE=routine` (a plugin's commands it runs only read; only its readonly scripts); the record `{"asked": "read", "used": "the folder session's: prepares and asks (reads and Echo's own edits; nothing sent; what would ask is refused)"}` |
| 5 | Switch "Claude acts on its own" on: load `{{base_url}}/` with a cookie jar (`curl -s -c "$D/jar" {{base_url}}/ > /dev/null`), then `curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data '{"cmd":"settings_set","key":"folder_skip_permissions","value":true}' {{base_url}}/ipc`; then `$E plugin call acme-asker run '{"tools":"read","dir":"r2"}'`; `grep -c '^--dangerously-skip-permissions$' "$D/run-args"`; `grep '^ECHOCTL_RUN_SESSION=' "$D/run-env"`; `cat "$ECHOCTL_HOME/.echo/plugins/acme-asker/runs/r2/permissions.json"` | `1` and `ECHOCTL_RUN_SESSION=acts` (no `ECHOCTL_RUN_MODE`): the user's one choice, whatever the plugin asked; the record `{"asked": "read", "used": "the folder session's: acts on its own (every permission prompt skipped)"}` |
| 6 | With the setting still on: `$E plugin call acme-asker run '{"tools":"none","dir":"r3"}'`; `grep -A1 '^--tools$' "$D/run-args"`; `grep -c -E '^--(dangerously-skip-permissions\|settings)$' "$D/run-args"`; `cat "$ECHOCTL_HOME/.echo/plugins/acme-asker/runs/r3/permissions.json"` | `--tools` followed by an empty line (no tools); `0`; the record `{"asked": "none", "used": "no tools"}` — a run that asked for no tools keeps none, whatever the setting |

## Expected Final State
Acme asker installed; its three runs started in the data folder (two as the folder session, one with no tools), refused by the fake; the setting is on.

## Teardown
- The setting off again (step 5's `curl` with `"value":false`); `$E plugin remove acme-asker --delete-data`; `$E plugin marketplace remove acme-local`; `mv "$D/bin/claude.fake" "$D/bin/claude"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `cat`, `grep`, `python3 -c` (reading the captured settings) and `curl` (the served window's own IPC, as the page sends it).
- That the routine's arguments are the folder session's but for `-p` and its prompt is the Rust test `claudejob::a_routine_and_a_yolo_task_run_as_the_folder_session_does`.
