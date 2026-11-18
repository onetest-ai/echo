---
id: TC-216
title: A plugin stops its own Claude run by its key from another process; the run fails "stopped" and is recorded so, a second run under a running key is refused
priority: high
type: functional
requirements: [F-PLUG-127]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, claude-run, fake-claude]
---

# TC-216: A plugin stops its own Claude run by its key

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Small data set built right before the run, nothing served: `tests/fixtures/make_data.py <dir> --ai fake`; every Bash call starts with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"` and aborts unless `$ECHOCTL_HOME` is inside `<dir>`; `echoctl` is `target/debug/echoctl`, built right before the run
- The test data's fake Claude Code: a run whose prompt holds "echo-test: keep running" goes on until it is stopped and writes its pid to `$D/fake-running.pid` (`make_data.py`, `FAKE_CLAUDE`); Run demo's `start` asks for such a run
- Run demo installed: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install run-demo --yes`

**Changes:** `<config dir>/plugins/` (the marketplace and the plugin); the plugin's store (`$ECHOCTL_HOME/.echo/plugins/run-demo/runs/index/`)

## Test Data

| Field | Value |
|---|---|
| Key | `index` |
| Prompt | "echo-test: keep running. Index the notes of Acme." (Run demo's own) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `(echoctl plugin call run-demo start '{"key":"index"}' > "$D/start.out" 2>&1 &)`; `tests/fixtures/wait.py 30 file "$D/fake-running.pid"` | "waited … for file …/fake-running.pid": the run's Claude Code is running |
| 2 | `echoctl plugin call run-demo start '{"key":"index"}'` | `{"error": "a run with key “index” is running already: stop it (ctx.claude.stop) or wait for it"}`, at once |
| 3 | `echoctl plugin call run-demo stop '{"key":"other"}'` | `false`: no run of its own under that key |
| 4 | `echoctl plugin call run-demo stop '{"key":"index"}'` | `true` |
| 5 | `tests/fixtures/wait.py 10 sh "grep -q stopped $D/start.out"; cat "$D/start.out"; kill -0 "$(cat "$D/fake-running.pid")"` | The first call answered `{"error": "stopped"}`; `kill -0` fails ("no such process"): the fake Claude Code is gone |
| 6 | `R="$ECHOCTL_HOME/.echo/plugins/run-demo/runs/index"; cat "$R/permissions.json"; tail -1 "$R/log.txt"` | The record holds `"stopped":true`; the log's last line ends "stopped by the plugin (ctx.claude.stop)" |
| 7 | `echoctl plugin call run-demo stop '{"key":"index"}'` | `false`: nothing runs under that key any more |

## Expected Final State
No Claude Code process of the case left; the run recorded as stopped; the key free again.

## Teardown
- `echoctl plugin remove run-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; remove `<dir>`

## Notes
- The stop comes from another process than the run (two `echoctl plugin call`s), as from the app while a terminal runs it. Another plugin's run under the same key going on, a key Echo can't take, a copy run's snapshot removed and its process group stopped: Rust, `jshost::folders::a_plugin_stops_its_own_run_by_its_key_and_another_plugins_run_under_that_key_goes_on`.
