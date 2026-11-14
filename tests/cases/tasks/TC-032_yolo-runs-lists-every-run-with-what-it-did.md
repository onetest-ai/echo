---
id: TC-032
title: Yolo runs lists every Yolo run newest first — a task's done with a change, a task's that didn't do it, a Needs you now item's that drafted a reply — each with when, how long, its task or item, thread, state, Claude's line, what it changed and drafted, its permissions and its log; the task opens its details; ⌘K opens the list
priority: medium
type: functional
requirements: [F-TASK-15]
module: tasks
run: agent
platform: macos
data: standard
ai: virtual
tags: [tasks, yolo, yolo-runs, virtual-model, claude-code]
---

# TC-032: Yolo runs lists every run with what it did

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; `{{base_url}}/` loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env` (abort if `ECHOCTL_HOME` isn't inside `<dir>`); Claude Code on `PATH` (only the model is the virtual one)
- "Claude acts on its own" **off** (the data's default)
- The script: `cp tests/fixtures/model-scripts/yolo-runs.json <dir>/model/model-script.json` and `: > <dir>/model/model-log.jsonl`. On Yolo's prompt for `echo://task/acme-rollout/T010` the model reads the task, closes it with `project work acme-rollout '{"op":"edit","node":"T010","set":{"state":"done"}}'` and answers "Booked the workshop room for Thursday (virtual model) and closed the task."; for `echo://task/globex-pilot/T001` it reads the task and answers "Didn't collect the user list: nobody has sent it yet (virtual model). Left for you: ask Kim Park for it." (no change); for the Needs you now item `echo://needs/mail:globex-redlines` it reads it, drafts a reply with `compose --to kim@globex.example --subject 'Re: Contract redlines for the Globex pilot' …` and answers "Drafted a reply to Kim Park about the redlines (virtual model). Left for you: send the draft."
- An IPC helper: `curl -s -c "$D/jar" {{base_url}}/ > /dev/null`; `ipc() { curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data "$1" {{base_url}}/ipc; }` (`D="$(dirname "$ECHOCTL_HOME")"`); `wait_run <id>`: read `$ECHOCTL_HOME/.echo/task-asks.json` every 2 s until the entry `<id>` is there and not `working` (a minute at most)

## Test Data

| Run | On | Thread |
|---|---|---|
| `ask-runs-1` | task T010 "Book the workshop room" | Acme rollout |
| `ask-runs-2` | task T001 "Collect the user list" | Globex pilot |
| `ask-runs-3` | Needs you now "Mail to you" (`mail:globex-redlines`) | Globex pilot |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | In one shell call, one after another: `ipc '{"cmd":"task_yolo","ask":"ask-runs-1","project":"acme-rollout","node":"T010","skill":"","text":""}'; wait_run ask-runs-1`, the same for `ask-runs-2` (`globex-pilot`, `T001`), then `ipc '{"cmd":"needs_yolo","ask":"ask-runs-3","id":"mail:globex-redlines","skill":"","text":""}'; wait_run ask-runs-3` | Each ends `done` |
| 2 | Navigate to `{{base_url}}/#today`, wait for "My work · this week"; click "Yolo runs" in its header (`[data-yolo-runs]`) | The address is `#yolo`; a page "Yolo runs" with "What Claude did on its own · the last 90 days" and a "Today" link; one row of filters: "All states", "All tasks and items", "All threads", "From", "To", "Changed or sent something", "Search what Claude said"; a count ("3 runs" on fresh data, more after other Yolo cases) |
| 3 | Read the list's rows (`.yo-run`, their `aria-label`s, in order) | Newest first: "Done: Mail to you", "Didn't do it: Collect the user list", "Done: Book the workshop room" (above any earlier run) |
| 4 | Read the row "Mail to you" | "Done", "Today HH:MM · took N s", "Prepared and asked"; "Needs you now Mail to you" (no link) with "Globex pilot"; its line "Drafted a reply to Kim Park about the redlines (virtual model). Left for you: send the draft."; "Drafted A draft opened in your mail app — “Re: Contract redlines for the Globex pilot” to kim@globex.example"; no "Changed" or "Sent" line |
| 5 | Read the row "Collect the user list" | "Didn't do it" (an amber dot), "took N s", "Prepared and asked"; the task "Collect the user list" as a link, "Globex pilot"; its line "Didn't collect the user list: nobody has sent it yet (virtual model). Left for you: ask Kim Park for it."; "Changed, sent and drafted nothing." |
| 6 | Read the row "Book the workshop room" | "Done" (a green dot); the task as a link, "Acme rollout"; "Booked the workshop room for Thursday (virtual model) and closed the task."; "Changed `echoctl project work acme-rollout {"op":"edit","node":"T010","set":{"state":"done"}}`" |
| 7 | Click "Log" on "Collect the user list" (`[data-yo-log="ask-runs-2"]`) | The button says "Hide the log"; under the row the run's log, first line "HH:MM:SS  Echo runs “Yolo: Collect the user list” on Virtual model, model virtual (asked: skip permissions; runs with the folder session's: prepares and asks …)", a "→ Bash … read echo://task/globex-pilot/T001" line and "Finished: ok"; the same text as `cat $ECHOCTL_CONFIG_DIR/yolo/ask-runs-2/log.txt` |
| 8 | Click "Hide the log", then the title "Book the workshop room" (`.yo-run [data-node="T010"]`) | The log is gone; the right panel's Details opens beside the list on T010 "Book the workshop room", Status "done", History "Echo's run set state" with "undo"; the address stays `#yolo` |
| 9 | Navigate to `{{base_url}}/#today`; press ⌘K (Ctrl+K), type "yolo runs", Enter | The palette lists "Yolo runs" under "Go to"; Enter opens `#yolo` with the same list |
| 10 | `ls $ECHOCTL_CONFIG_DIR/yolo/ask-runs-1/`; `python3 -c "import json,os; print(json.load(open(os.environ['ECHOCTL_CONFIG_DIR']+'/yolo/ask-runs-1/run.json')))"` | `run.json` beside `log.txt`, `prompt.md`, `answer.md`, `permissions.json`, `actions.jsonl`; the record has `title` "Book the workshop room", `project` "acme-rollout", `node` "T010", `thread` "acme-rollout", `started`, `ended`, `state` "done" and the `say` of step 6 |

## Expected Final State
Three runs on Yolo runs, newest first, each saying what it did; T010 is done by Echo's run; the reply to the redlines is a draft; nothing was sent.

## Teardown
- None (the scenario stops the data), else `tests/fixtures/serve.sh down <dir>`

## Notes
- "Didn't do it" is a run that finished but changed, sent and drafted nothing with Echo's CLI; "Done" is one that did at least one of them (a draft counts).
- A run's record is `<config>/yolo/<id>/run.json` (#603); a run of an older Echo without one shows from its outcome in `task-asks.json` while that holds it (the last 10). Runs older than 90 days drop off the list; their folders go when the next run starts (Rust: `yolo::tests`).
