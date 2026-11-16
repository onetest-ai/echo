---
id: TC-031
title: With "Claude acts on its own" off, Yolo on the virtual model prepares and asks — its send is refused before it reaches Echo, its mail is drafted, another thread's work refused, its task closed as Echo's run, and its record lists the draft and the change, no send
priority: high
type: functional
requirements: [F-PLUG-35, F-M365-08, F-TASK-12]
module: tasks
run: agent
platform: macos
data: standard
ai: virtual
tags: [tasks, yolo, virtual-model, claude-code, permissions]
auto: [tests/auto/specs/S-33.spec.mjs]
---

# TC-031: Yolo preparing and asking drafts instead of sending

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; `{{base_url}}/` loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`; Claude Code on `PATH` (only the model is the virtual one)
- "Claude acts on its own" **off** (Settings → AI & minutes; the data's default — after TC-026 it is switched back off)
- The script: `cp tests/fixtures/model-scripts/yolo-asks.json <dir>/model/model-script.json` and an empty request log (`: > <dir>/model/model-log.jsonl`). On Yolo's prompt for `echo://task/globex-pilot/T009` the model, by Bash with the CLI's full path from the prompt, in turn: reads its task, sends a mail with `m365 mail send … --yes`, drafts it with `compose … --body …` instead, reads another thread's work with `project work acme-rollout`, closes T009 with `project work globex-pilot '{"op":"edit","node":"T009","set":{"state":"done"}}'`; then answers "Drafted the welcome mail to Kim Park (virtual model); sending it was refused. Left for you: send the draft."
- An IPC helper for the served window: `curl -s -c "$D/jar" {{base_url}}/ > /dev/null`; `ipc() { curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data "$1" {{base_url}}/ipc; }` (`D="$(dirname "$ECHOCTL_HOME")"`)

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `ipc '{"cmd":"task_yolo","ask":"ask-asks-1","project":"globex-pilot","node":"T009","skill":"","text":""}'`; read `$ECHOCTL_HOME/.echo/task-asks.json` every 5 s until the entry `ask-asks-1` is no longer `working` (a minute at most) | `state` "done", `say` the script's answer |
| 2 | `python3 -c "import json,os; L=[json.loads(l) for l in open(os.environ['ECHOCTL_HOME']+'/../model/model-log.jsonl')]; e=[x for x in L if x.get('rule')==0]; print([b.get('is_error') or False for x in e for b in (x['messages'][-1]['content'] if isinstance(x['messages'][-1]['content'],list) else []) if b.get('type')=='tool_result'])"` | `[False, True, False, True, False]`: the read, the draft and the close went through; the send was refused by Claude Code itself (nothing allows a send without a prompt), never reaching Echo; reading `acme-rollout`'s work was refused by Echo — Yolo reads its own task's thread only, the setting on or off |
| 3 | `tail -1 <dir>/mail-drafts.log`; `cat $ECHOCTL_CONFIG_DIR/yolo/ask-asks-1/permissions.json`; `cat $ECHOCTL_CONFIG_DIR/yolo/ask-asks-1/actions.jsonl` | A `mailto:kim@globex.example?subject=Welcome%20to%20the%20Globex%20pilot…` line (the draft, no mail app opened); `used` "the folder session's: prepares and asks (…)"; two lines, `draft` (`compose --to kim@globex.example …`) and `change` (`project work globex-pilot …`), no `send` |
| 4 | `target/debug/echoctl project ledger globex-pilot tree` (T009); `tail -1 $ECHOCTL_HOME/projects/globex-pilot/ledger/events.jsonl` | T009 `state` "done"; the last event `"by": "run"`, `"op": "edit"`, `"node": "T009"`, `"set": {"state": "done"}` — Echo's run's, not yours |

## Expected Final State
T009 is done, by Echo's run; the welcome mail is a draft; nothing was sent; "Claude acts on its own" is off.

## Teardown
- None (S-33 stops the data)
