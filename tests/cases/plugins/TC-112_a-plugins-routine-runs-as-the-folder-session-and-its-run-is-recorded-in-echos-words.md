---
id: TC-112
title: A plugin's routine runs through Claude Code as the folder session on the default AI profile with the prompt it ships, in the routines' permission setting; its run is recorded in Echo's words and counts, and its outcome — the answer and what it changed and sent — is the user's
priority: high
type: functional
requirements: [F-PLUG-77, F-PLUG-78]
module: plugins
run: agent
platform: macos
data: standard
ai: fake
tags: [plugins, routines, claude-code, permissions]
---

# TC-112: A plugin's routine runs as the folder session, and its run is recorded in Echo's words

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data with the fake Claude Code, built right before the run, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl` — inside the repository: Echo finds its own plugin from there — built right before) after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- **A fake `claude` that writes down how Echo starts it**, in front of the data's fake (never a real account), as plugins/TC-038's preconditions write it: `mv "$D/bin/claude" "$D/bin/claude.fake"`, then `$D/bin/claude` (`chmod +x`) writing `run-args`, `run-cwd`, `run-env` and `run-settings.json` and running `claude.fake`
- **The fake's answer for the routine's prompt** (a made-up result line, chosen by the phrase in its prompt): `printf 'ROUTINE-DEMO mail-notes\t%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"Kept 2 notes from 3 mails","total_cost_usd":0.0123,"duration_ms":900,"num_turns":2,"usage":{"input_tokens":1200,"output_tokens":80,"cache_read_input_tokens":300,"cache_creation_input_tokens":0}}' >> "$D/fake-answers.tsv"`
- The test marketplace: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install routine-demo --yes`
- The served window's cookie for the IPC step: `curl -s -c "$D/jar" {{base_url}}/ > /dev/null`

**Changes:** the installed plugins, `<config dir>/jobs-runs/`, `$D/run-*`, `$D/fake-answers.tsv`, `config.json` (step 5)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E jobs run routine-demo.mail-notes` (it runs off or on: the user asked) | The run: `state` "done", `ok` true, `summary` "done", `input` 1200, `output` 80, `cache_read` 300, `cost_usd` 0.0123, `calls` 1, `changes` 0, `sends` 0, a `run` (its folder's name), `started`, `ended`, `trial` false |
| 2 | `cat "$D/run-cwd"`; `grep -E '^--(settings\|permission-mode\|dangerously-skip-permissions)$' -A1 "$D/run-args"`; `grep -c 'This is the routine “Mail → notes” of the plugin “Routine demo”' "$D/run-args"`; `grep -c '^ROUTINE-DEMO mail-notes: read the mail' "$D/run-args"`; `grep -E '^ECHOCTL_RUN_SESSION=' "$D/run-env"` | The data folder (`$ECHOCTL_HOME`); a `--settings` file and `--permission-mode` then `auto` (the routines' setting, Auto), no `--dangerously-skip-permissions`; `1` (Echo's lines name it and its plugin); `1` (the prompt its package ships); `ECHOCTL_RUN_SESSION=acts` |
| 3 | `$E jobs outcome routine-demo.mail-notes` | `answer` "Kept 2 notes from 3 mails"; `actions` []; `permissions` {"asked": "setting", "used": "the folder session's: acts on its own (auto mode: Claude Code decides each action)"}; its `log`'s first line holds `Echo runs “Routine demo · Mail → notes”`; `run` the run of step 1 |
| 4 | `$E jobs routines --runs routine-demo.mail-notes`; `$E jobs routines` | One run, as step 1's, with no "Kept 2 notes" anywhere (the model's text is the user's, never in a record a plugin reads). The routine's `last` that run; `month.runs` 1, `month.cost` 0.0123; `next_run` null (it is off) |
| 5 | Routines on "Skip all permission checks": `curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data '{"cmd":"settings_set","key":"routine_permissions","value":"skip"}' {{base_url}}/ipc`; `$E jobs run routine-demo.mail-notes`; `grep -c '^--dangerously-skip-permissions$' "$D/run-args"`; `$E jobs outcome routine-demo.mail-notes`; then the same `curl` with `"value":"auto"` | `state` "done"; `1`; the newest run's `permissions.used` "the folder session's: acts on its own (every permission check skipped)" |
| 6 | Without its answer: `grep -v '^ROUTINE-DEMO' "$D/fake-answers.tsv" > "$D/fa"; mv "$D/fa" "$D/fake-answers.tsv"`; `$E jobs run routine-demo.mail-notes; echo "exit $?"`; `$E jobs routines --runs routine-demo.mail-notes` | `Error:` with the fake's "test data: Claude Code is a fake here"; `exit 1`. The newest run: `state` "failed", `error` "the run failed: its log says why"; three runs |

## Expected Final State
Routine demo installed, its routine off with three runs (two done, one failed); routines on Auto.

## Teardown
- `$E plugin remove routine-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `mv "$D/bin/claude.fake" "$D/bin/claude"`; `rm -f "$D/jar" "$D"/run-*`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `cat`, `grep`, `mv`, `printf`, `rm` and `curl` (the served window's own IPC, as the page sends it).
- A routine's own `skip` honoured only when its plugin's sheet has `claude.skip`, `tools: none` keeping no tools, the summary's counts of changes and sends, the model setting mapped by the profile, and the last 20 run folders kept: the Rust test `plugins::routines::a_routine_runs_as_the_folder_session_on_the_default_profile_and_is_recorded_in_echos_words`.
- That it runs every N minutes in the app (not at night when it pauses then) is the app's scheduler, which never runs on test data: `plugins::routines::a_routine_is_due_every_n_minutes_from_its_last_run_off_and_at_night_never`.
