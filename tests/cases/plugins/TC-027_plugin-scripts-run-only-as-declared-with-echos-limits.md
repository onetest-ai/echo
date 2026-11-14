---
id: TC-027
title: A plugin's scripts (uses.scripts) are named on the sheet with their time and files' hash; the host half runs one by ctx.scripts.run, Echo's sessions through the launcher at its path, both without Echo's or Claude's environment; a read-only routine is given only the readonly one, its guard refusing an interpreter, a pipe or another script; its output reaches the host half by a plugin command; an undeclared script is refused; a change in its skill folder asks again
priority: critical
type: functional
requirements: [F-PLUG-25]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, sessions, skills, scripts, permissions, untrusted-text, guard]
---

# TC-027: A plugin's scripts run only as declared, with Echo's limits

**Module:** Plugins | **Priority:** Critical | **Type:** Functional

## Preconditions
- Fresh standard test data with the fake Claude Code: `python3 tests/fixtures/make_data.py <dir> --set standard --ai fake` (an empty `<dir>`); CLI steps run in Bash after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`; `echoctl` is `target/debug/echoctl` inside the repository (Echo finds its own plugin from there), built right before the run. No window step
- **A fake `claude` that writes down what Echo starts it with**, in front of the data's fake (never a real account): `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # A run (not --version, not auth) writes its arguments, one a line, and the plugin commands and scripts its guard is given to $D/run-args.
  d="$(dirname "$0")/.."
  case "$1" in --version|auth) ;; *) { printf '%s\n' "$@"; echo "RUN_PLUGIN_COMMANDS=$ECHOCTL_RUN_PLUGIN_COMMANDS"; echo "RUN_PLUGIN_SCRIPTS=$ECHOCTL_RUN_PLUGIN_SCRIPTS"; } > "$d/run-args";; esac
  exec "$d/bin/claude.fake" "$@"
  ```
- A made-up plugin **Acme kit** in a local marketplace `acme-local`: package skill `staffing` with a readonly script `scripts/read.sh` (prints made-up staffing JSON for the project code it is given, and what of its environment it sees), a script `scripts/tidy.sh` (not readonly, 20 minutes), a script `scripts/slow.sh` (sleeps) and a file `scripts/stray.sh` it doesn't declare. The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$D/market"; K="$M/plugins/acme-kit"; S="$ECHOCTL_CONFIG_DIR/plugins/session/acme-kit"; R="$S/skills/staffing/scripts/read.sh"
mkdir -p "$K/skills/staffing/scripts"
echo '{"name":"acme-local","plugins":[{"id":"acme-kit","source":"./plugins/acme-kit"}]}' > "$M/echo-marketplace.json"
kit() { cat > "$K/echo-plugin.json" <<EOF
{"id":"acme-kit","name":"Acme kit","version":"$1","summary":"A made-up plugin for tests","main":"main.js","skills":["skills/staffing"],"agent":{"prefix":"acme"},
 "uses":{"claude":{},"sessions":{"commands":["put"]},"scripts":[{"skill":"staffing","script":"scripts/read.sh","readonly":true},{"skill":"staffing","script":"scripts/tidy.sh","timeout":1200},{"skill":"staffing","script":"scripts/slow.sh"}]}}
EOF
}
kit 0.1.0
printf -- '---\nname: staffing\ndescription: Reads the staffing of an Acme project.\n---\n\nRun scripts/read.sh <project code>.\n' > "$K/skills/staffing/SKILL.md"
printf '#!/bin/sh\necho "{\\"project\\": \\"$1\\", \\"people\\": 3, \\"script\\": \\"$ECHO_PLUGIN_SCRIPT\\", \\"profile\\": \\"${ECHOCTL_P_TOKEN:-none}\\", \\"claude\\": \\"${CLAUDE_CODE_ENTRYPOINT:-none}\\"}"\n' > "$K/skills/staffing/scripts/read.sh"
printf '#!/bin/sh\necho tidied\n' > "$K/skills/staffing/scripts/tidy.sh"
printf '#!/bin/sh\necho stray\n' > "$K/skills/staffing/scripts/stray.sh"
printf '#!/bin/sh\nsleep 299\n' > "$K/skills/staffing/scripts/slow.sh"
cat > "$K/main.js" <<'EOF'
export function apply(ctx) {
  const msg = (e) => String((e && e.message) || e)
  ctx.command('fetch', async (a) => { try { return await ctx.scripts.run('staffing', a.script || 'scripts/read.sh', a.args || [], a.opts) } catch (e) { return { error: msg(e) } } })
  ctx.command('put', (a) => { ctx.store.write('cache/staffing.json', JSON.stringify(a)); return { ok: true } })
  ctx.command('got', () => JSON.parse(ctx.store.read('cache/staffing.json')))
  ctx.command('routine', async () => { try { await ctx.claude.run({ prompt: 'Read the staffing of ACME-1', tools: 'read' }); return { ok: true } } catch (e) { return { error: msg(e) } } })
}
EOF
```
- The marketplace is added: `echoctl plugin marketplace add "$M" --name acme-local`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, `plugins/session/acme-kit/`), the plugin's store in `<dir>` (`cache/staffing.json`), `$D/run-args`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-kit --yes` | The sheet it prints has the line `Runs programs from its package on this computer with your rights — they can reach the network and anything you can: “staffing/scripts/read.sh” (read-only routines may run it; Echo can't check it only reads), “staffing/scripts/tidy.sh” (up to 20 min), “staffing/scripts/slow.sh” (their files: ` + 8 hex characters + `)`; no `stray.sh` in it; `Installed Acme kit 0.1.0` |
| 2 | `cat "$R"`; `ls -l "$K/skills/staffing/scripts" "$ECHOCTL_CONFIG_DIR/plugins/installed/acme-kit/0.1.0/skills/staffing/scripts"` | `$R` is a short `#!/bin/sh` launcher whose last line `exec`s Echo's CLI (`…/plugins/session/acme-kit/scripts/echoctl`) with `plugin script 'acme-kit' 'staffing' 'scripts/read.sh' -- "$@"`; in the marketplace every script is `-rw-r--r--` (written without the executable bit); in the installed package `read.sh`, `tidy.sh` and `slow.sh` are `-rwxr-xr-x` (Echo made the declared ones runnable) and `stray.sh` stays `-rw-r--r--` |
| 3 | `echoctl plugin call acme-kit fetch '{"args":["ACME-1"]}'` (the host half) | The answer has `"status": 0`, `"stderr": ""` and `stdout` the line `{"project": "ACME-1", "people": 3, "script": "1", "profile": "none", "claude": "none"}` (`echoctl plugin call` prints a string's line break as a line break) — `script` 1: Echo told it nobody is there to answer |
| 4 | `echoctl plugin call acme-kit fetch '{"args":["$(id)","a b; rm -rf ~"]}'` | `"status": 0` and `stdout` starts `{"project": "$(id)",` — the literal text, one argument: no shell ran it |
| 5 | `ECHOCTL_P_TOKEN=leak CLAUDE_CODE_ENTRYPOINT=cli "$R" 'ACME 2'; echo "exit=$?"` (as a session runs it: its launcher by its path) | Prints `{"project": "ACME 2", "people": 3, "script": "1", "profile": "none", "claude": "none"}` — a profile's value and Claude Code's variables don't reach the script — then `exit=0` |
| 6 | `echoctl plugin script acme-kit staffing scripts/stray.sh; echo "exit=$?"`; then `echoctl plugin call acme-kit fetch '{"script":"scripts/stray.sh"}'`; then `echoctl plugin call acme-kit fetch '{"opts":{"timeout":301}}'` | `Error: acme-kit didn't declare the script “staffing/scripts/stray.sh” when it was installed`, `exit=1`; the host half's answer `error` says the same; the third `error` contains `staffing/scripts/read.sh may take 300 s at most, not 301` |
| 7 | `ECHOCTL_RUN_MODE=routine "$S/skills/staffing/scripts/tidy.sh"; echo "exit=$?"` (inside a read-only routine) | `Error: staffing/scripts/tidy.sh isn't marked readonly: a read-only routine doesn't run it`, `exit=1` |
| 8 | `echoctl plugin call acme-kit routine` (it asks for read-only tools); then `L="$ECHOCTL_HOME/.plugins/acme-kit/.claude/skills/acme-staffing/scripts"; grep -c "Bash($L/read.sh:\*)" "$D/run-args"; grep -c "Bash($L/tidy.sh:\*)" "$D/run-args"; grep -c 'plugin call acme-kit put:\*)' "$D/run-args"; grep ^RUN_PLUGIN_SCRIPTS= "$D/run-args"` | The call answers an `error` from the data's fake Claude Code (`test data: Claude Code is a fake here`), not a refusal; a routine runs as the folder session (#590): with "Claude acts on its own" off (the data's) it prepares and asks: of its scripts only the one marked readonly is allowed, by its launcher's path in the data folder's `.plugins/acme-kit/` (`read.sh` 1, `tidy.sh` 0), its command `put` allowed (at least 1: run with the read-only marker, it changes nothing a plugin couldn't in a read-only routine); `RUN_PLUGIN_SCRIPTS=` empty: no guard |
| 9 | What the routine's Claude would do, step by step, with what it was allowed in step 8: `out=$("$R" ACME-1)`; `echoctl plugin call acme-kit put "$out"`; `echoctl plugin call acme-kit got` | `{"ok": true}`; then JSON with `"project": "ACME-1"` and `"people": 3`: the script's output reached the host half through its plugin command |
| 10 | A version changing only a file of the skill folder: `kit 0.2.0; printf '#!/bin/sh\necho tidied twice\n' > "$K/skills/staffing/scripts/tidy.sh"; echoctl plugin marketplace update acme-local; echoctl plugin update acme-kit` (no `--yes`) | The update is refused: the sheet it prints marks the `Runs programs from its package…` line and the `Tells Claude in Echo how to work with it (the skill “staffing”)…` line `(new)` (both `files:` hashes cover the skill folder; no other line is new) and it ends `Error: run again with --yes to agree`; `echoctl plugin list` still shows `acme-kit` `0.1.0` |
| 11 | `echoctl plugin update acme-kit --yes`; then `echoctl plugin call acme-kit fetch '{"script":"scripts/tidy.sh"}'` | `Updated Acme kit to 0.2.0`; the answer's `stdout` is `tidied twice` |
| 12 | In one Bash call: `"$S/skills/staffing/scripts/slow.sh" & B=$!; sleep 1; echoctl plugin call acme-kit fetch '{"script":"scripts/slow.sh"}'` (a session's run in its own process, then the host half's) | The host half's answer `error` is `staffing/scripts/slow.sh is already running`: one run of a script at a time, across processes |
| 13 | In the same Bash call: `echoctl plugin remove acme-kit --delete-data; wait $B; echo "exit=$?"; pgrep -f "sleep 299" \|\| echo gone`; then `ls "$ECHOCTL_CONFIG_DIR/plugins/session"` | The removal stopped the session's run (`exit=` not 0, within seconds, not 299 s) with what it started: `gone`; no `acme-kit` entry in the listing (`ls` may fail with "No such file or directory") |

## Expected Final State
Acme kit is removed with its session folder. While installed, its scripts ran only as declared — by its host half and through Echo's launcher, never with a profile's value or Claude Code's variables — and its routine ran as the folder session, allowed every declared script; the readonly script's output reached the host half through its plugin command.

## Teardown
- `echoctl plugin marketplace remove acme-local`; `mv "$D/bin/claude.fake" "$D/bin/claude"`; remove the scratch folder's `market` directory (`rm -r "$M"`) and `<dir>`'s parent when the run is over

## Notes
- `platform: macos`: the scripts are `/bin/sh` with Unix paths; on Windows a `.ps1` runs with PowerShell and another script with the interpreter its `#!` line names (`plugins::scripts::command_for`), not run here.
- A real model is never run (step 8 ends in the data's fake): what the routine is handed is checked in step 8, and step 9 runs those allowed commands by hand.
- A script's time running out (stopped with what it started, `timed_out`), output past 1 MB (`truncated`) and two runs of the same script from one process are the Rust test `jshost::a_plugins_package_skill_reaches_the_sessions_and_its_declared_script_runs`.
