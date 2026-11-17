---
id: TC-030
title: Yolo on a task runs Claude Code as the folder session does — in the data folder, with its settings and hooks (the core among them), acting on its own in Yolo's own permission mode (Auto by default, or Skip all permission checks) unless Claude in the window asks each time — and keeps what it asked for in its record
priority: high
type: functional
requirements: [F-TASK-12, F-PLUG-35, F-SET-24]
module: tasks
run: agent
platform: macos
data: standard
ai: fake
tags: [tasks, yolo, claude-code, permissions]
---

# TC-030: Yolo runs as the folder session

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data with the fake Claude Code, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; CLI steps in Bash after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`; `E=target/debug/echoctl`, built right before the run; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The fake `claude` that writes down how Echo starts it, as in `plugins/TC-038` (its `$D/bin/claude` in front of `$D/bin/claude.fake`)
- A cookie for the served window's IPC: `curl -s -c "$D/jar" {{base_url}}/ > /dev/null`; `ipc() { curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data "$1" {{base_url}}/ipc; }`

**Changes:** `<config dir>/yolo/`, `<home>/.echo/task-asks.json`, `$D/run-*`, `config.json` (steps 3–4)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `ipc '{"cmd":"task_yolo","ask":"ask-1","project":"globex-pilot","node":"T005","skill":"","text":""}'`; wait until `$ECHOCTL_HOME/.echo/task-asks.json`'s entry `ask-1` is no longer `working` (every 2 s, up to 30 s) | Its `state` is `failed` (the data's fake refuses the run): it started |
| 2 | `cat "$D/run-cwd"`; `grep -c -E '^--(dangerously-skip-permissions\|permission-mode)$' "$D/run-args"`; `python3 -c "import json,sys; h=json.load(open(sys.argv[1]))['hooks']; print([x['command'].split()[-2:] for e in h['SessionStart'] for x in e['hooks']])" "$D/run-settings.json"`; `cat "$ECHOCTL_CONFIG_DIR/yolo/ask-1/permissions.json"`; `head -1 "$ECHOCTL_CONFIG_DIR/yolo/ask-1/log.txt"` | `$ECHOCTL_HOME`; `0` (Claude in the window asks each time, as on test data: Yolo prepares and asks, no mode set); `view-hook` and `core --hook`; `{"asked": "skip", "used": "the folder session's: prepares and asks (reads and Echo's own edits; nothing sent; what would ask is refused)"}`; the log line ends `(asked: skip permissions; runs with the folder session's: prepares and asks (reads and Echo's own edits; nothing sent; what would ask is refused))` |
| 3 | `ipc '{"cmd":"settings_set","key":"folder_permissions","value":"auto"}'`; `ipc '{"cmd":"task_yolo","ask":"ask-2","project":"globex-pilot","node":"T005","skill":"","text":""}'`; wait for `ask-2` as in step 1; `grep -c '^--dangerously-skip-permissions$' "$D/run-args"`; `grep -A1 '^--permission-mode$' "$D/run-args"`; `grep '^ECHOCTL_RUN_SESSION=' "$D/run-env"`; `cat "$ECHOCTL_CONFIG_DIR/yolo/ask-2/permissions.json"` | `0`; `--permission-mode` then `auto`; `ECHOCTL_RUN_SESSION=acts`: Claude in the window not asking each time, Yolo acts on its own in Claude Code's auto mode, its default (its own task's and thread's limits stay); the record `{"asked": "skip", "used": "the folder session's: acts on its own (auto mode: Claude Code decides each action)"}` |
| 4 | `ipc '{"cmd":"settings_set","key":"yolo_permissions","value":"skip"}'`; `ipc '{"cmd":"task_yolo","ask":"ask-3","project":"globex-pilot","node":"T005","skill":"","text":""}'`; wait for `ask-3` as in step 1; `grep -c '^--dangerously-skip-permissions$' "$D/run-args"`; `grep -c '^--permission-mode$' "$D/run-args"`; `head -1 "$ECHOCTL_CONFIG_DIR/yolo/ask-3/log.txt"` | `1`; `0`; the log line ends `runs with the folder session's: acts on its own (every permission check skipped))` — Yolo's own setting |

## Expected Final State
Three Yolo runs started (refused by the fake) as the folder session; Claude in the window on Auto, Yolo on Skip all permission checks.

## Teardown
- `ipc '{"cmd":"settings_set","key":"folder_permissions","value":"ask"}'`; `ipc '{"cmd":"settings_set","key":"yolo_permissions","value":"auto"}'`; `mv "$D/bin/claude.fake" "$D/bin/claude"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `cat`, `grep`, `head`, `python3 -c` and `curl` (the served window's own IPC, as the page sends it).
