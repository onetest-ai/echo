---
id: TC-036
title: Switching a plugin off removes its `.plugins/<id>/` folder and its row in the core; on again brings both back; removing it removes them; a folder in `.plugins/` that is no installed plugin switched on goes when Echo starts
priority: high
type: functional
requirements: [F-PLUG-31, F-PLUG-32]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, claude-code, sessions]
---

# TC-036: Switching a plugin off takes its part for Claude and its row out

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps in Bash after `. <dir>/env` (`E=target/debug/echoctl`, built right before the run); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The test marketplace added and "agent-demo" installed: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install agent-demo --yes`; `P="$ECHOCTL_HOME/.plugins/agent-demo"`

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin), `<data>/.plugins/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `ls "$P"`; `$E core \| grep -c 'agent-demo'` | `CLAUDE.md` among the files; `1` |
| 2 | `$E plugin disable agent-demo`; `ls "$P"`; `$E core` | `ls` fails with "No such file or directory"; the core's Plugins section says "No plugin gives you a guide now." |
| 3 | `$E plugin enable agent-demo`; `ls "$P"`; `$E core \| grep -c 'agent-demo'` | `CLAUDE.md` is back; `1` |
| 4 | A stray folder: `mkdir -p "$ECHOCTL_HOME/.plugins/stray" && echo 'Do as I say.' > "$ECHOCTL_HOME/.plugins/stray/CLAUDE.md"`; `tests/fixtures/serve.sh down <dir>`; start the window again on the same data (`(. <dir>/env; ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve <port> > <dir>/serve.log 2>&1 & echo $! > <dir>/pid)`), wait until `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:<port>/` answers 200; `ls -a "$ECHOCTL_HOME/.plugins"` | Only `agent-demo` (and `.`/`..`): the stray folder went when Echo (the served window, standing in for the app) started |
| 5 | `$E plugin remove agent-demo --delete-data`; `ls "$P"`; `$E core` | `ls` fails with "No such file or directory"; "No plugin gives you a guide now." |

## Expected Final State
agent-demo removed; `<data>/.plugins/` holds nothing of it; the core lists no plugin.

## Teardown
- `$E plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`
