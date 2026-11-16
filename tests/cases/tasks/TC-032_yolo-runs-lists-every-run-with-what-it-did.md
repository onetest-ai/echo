---
id: TC-032
title: Yolo runs lists every Yolo run newest first — a task's done with a change, a task's done with nothing through Echo, a Needs you now item's that drafted a reply, a failed one — each with when, how long, its task or item, thread, state from its outcome, Claude's line, what it did through Echo counted, its drafts, its permissions and its Details (commands and log); the task opens its details; ⌘K opens the list
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
- The script: `cp tests/fixtures/model-scripts/yolo-runs.json <dir>/model/model-script.json` and `: > <dir>/model/model-log.jsonl`. On Yolo's prompt for `echo://task/acme-rollout/T010` the model reads the task, closes it with `project work acme-rollout '{"op":"edit","node":"T010","set":{"state":"done"}}'` and answers "Booked the workshop room for Thursday (virtual model) and closed the task."; for `echo://task/globex-pilot/T001` it reads the task and answers "Didn't collect the user list: nobody has sent it yet (virtual model). Left for you: ask Kim Park for it." (no change); for the Needs you now item `echo://needs/mail:globex-redlines` it reads it, drafts a reply with `compose --to kim@globex.example --subject 'Re: Contract redlines for the Globex pilot' …` and answers "Drafted a reply to Kim Park about the redlines (virtual model). Left for you: send the draft."; for `echo://task/acme-rollout/T016` the model's API answers an error 400 ("virtual model: this run fails (Yolo runs' Failed)"), so that run fails
- An IPC helper: `curl -s -c "$D/jar" {{base_url}}/ > /dev/null`; `ipc() { curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data "$1" {{base_url}}/ipc; }` (`D="$(dirname "$ECHOCTL_HOME")"`); `wait_run <id>`: read `$ECHOCTL_HOME/.echo/task-asks.json` every 2 s until the entry `<id>` is there and not `working` (a minute at most)

## Test Data

| Run | On | Thread |
|---|---|---|
| `ask-runs-1` | task T010 "Book the workshop room" | Acme rollout |
| `ask-runs-2` | task T001 "Collect the user list" | Globex pilot |
| `ask-runs-3` | Needs you now "Mail to you" (`mail:globex-redlines`, "Contract redlines for the Globex pilot") | Globex pilot |
| `ask-runs-4` | task T016 "Map the user groups" | Acme rollout |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | In one shell call, one after another: `ipc '{"cmd":"task_yolo","ask":"ask-runs-1","project":"acme-rollout","node":"T010","skill":"","text":""}'; wait_run ask-runs-1`, the same for `ask-runs-2` (`globex-pilot`, `T001`), `ipc '{"cmd":"needs_yolo","ask":"ask-runs-3","id":"mail:globex-redlines","skill":"","text":""}'; wait_run ask-runs-3`, then `ask-runs-4` (`acme-rollout`, `T016`); last, a run id of 70 letters: `ipc '{"cmd":"task_yolo","ask":"aaaa…(70 a)","project":"acme-rollout","node":"T016","skill":"","text":""}'` | The first three end `done`, `ask-runs-4` `failed`; the last is refused at once: its reply is the notice "a bad run id", and no folder of that name is in `$ECHOCTL_CONFIG_DIR/yolo/` |
| 2 | Navigate to `{{base_url}}/#today`, wait for "My work · this week"; click "Yolo runs" in its header (`[data-yolo-runs]`) | The address is `#yolo`; a page "Yolo runs" with "What Claude did on its own · the last 90 days" and a "Today" link; one row of filters: "All states", "All tasks and items", "All threads", "From", "To", "Changed or sent through Echo", "Search what Claude said"; a count ("4 runs" on fresh data, more after other Yolo cases) |
| 3 | Read the list's rows (`.yo-run`, their `aria-label`s, in order) | Newest first: "Failed: Map the user groups", "Done: Mail to you · Contract redlines for the Globex pilot", "Done: Collect the user list", "Done: Book the workshop room" (above any earlier run); no other state than Working, Done or Failed anywhere |
| 4 | Read the row "Mail to you · Contract redlines for the Globex pilot" | "Done", "Today HH:MM · took N s", "Prepared and asked"; "Needs you now Mail to you · Contract redlines for the Globex pilot" (no link) with "Globex pilot"; its line "Drafted a reply to Kim Park about the redlines (virtual model). Left for you: send the draft."; "Drafted A draft opened in your mail app — “Re: Contract redlines for the Globex pilot” to kim@globex.example"; no "Through Echo:" line and no "Nothing through Echo" |
| 5 | Read the row "Collect the user list" | "Done" (a green dot: the run's own outcome), "took N s", "Prepared and asked"; the task "Collect the user list" as a link, "Globex pilot"; its line "Didn't collect the user list: nobody has sent it yet (virtual model). Left for you: ask Kim Park for it."; "Nothing through Echo" (a neutral note) |
| 6 | Read the rows "Book the workshop room" and "Map the user groups" | "Book the workshop room": "Done"; the task as a link, "Acme rollout"; "Booked the workshop room for Thursday (virtual model) and closed the task."; "Through Echo: 1 change" (the command itself is not on the row). "Map the user groups": "Failed" (a red dot), its line "Claude Code failed: API Error: 400 virtual model: this run fails (Yolo runs' Failed)…" |
| 7 | Click "Details" on "Book the workshop room" (`[data-yo-log="ask-runs-1"]`) | The button says "Hide details"; under the row "Changed `echoctl project work acme-rollout {"op":"edit","node":"T010","set":{"state":"done"}}`" and the run's log, first line "HH:MM:SS  Echo runs “Yolo: Book the workshop room” on Virtual model, model virtual (asked: skip permissions; runs with the folder session's: prepares and asks …)", a "→ Bash … project work acme-rollout …" line and "Finished: ok"; the same text as `cat $ECHOCTL_CONFIG_DIR/yolo/ask-runs-1/log.txt`; no "The log is long" note |
| 8 | Click "Hide details", then the title "Book the workshop room" (`.yo-run [data-node="T010"]`) | The details are gone; the right panel's Details opens beside the list on T010 "Book the workshop room", Status "done", History "Echo's run set state" with "undo"; the address stays `#yolo` |
| 9 | Navigate to `{{base_url}}/#today`; press ⌘K (Ctrl+K), type "yolo runs", Enter | The palette lists "Yolo runs" under "Go to"; Enter opens `#yolo` with the same list |
| 10 | `ls $ECHOCTL_CONFIG_DIR/yolo/ask-runs-1/`; `python3 -c "import json,os; print(json.load(open(os.environ['ECHOCTL_CONFIG_DIR']+'/yolo/ask-runs-1/run.json')))"` | `run.json` beside `log.txt`, `prompt.md`, `answer.md`, `permissions.json`, `actions.jsonl`; the record has `title` "Book the workshop room", `project` "acme-rollout", `node` "T010", `thread` "acme-rollout", `started`, `ended`, `state` "done", `pid` and `echo` (the serving Echo's process and its start stamp) and the `say` of step 6 |

## Expected Final State
Four runs on Yolo runs, newest first, each saying what it did; T010 is done by Echo's run; the reply to the redlines is a draft; nothing was sent.

## Teardown
- None (the scenario stops the data), else `tests/fixtures/serve.sh down <dir>`

## Notes
- A run's state is its own outcome: Working, Done or Failed. Echo notes only what a run does with its CLI (an acting run may also change things with Write, Edit, MCP tools or scripts), so a run with no change, send or draft through Echo gets the note "Nothing through Echo", not a verdict; an older run Echo didn't follow gets no note.
- A run's record is `<config>/yolo/<id>/run.json` (#603), with the `pid` and start stamp (`echo`) of the Echo running it: a working run another Echo started (gone, or its pid now another process's) shows Failed at once; any working run past its 20-minute limit with its log quiet for 5 minutes shows Failed (a laptop asleep mid-run writes on and stays Working). A run of an older Echo without a record shows from its outcome in `task-asks.json` while that holds it (the last 10). Runs older than 90 days drop off; their folders go when the next run starts (Rust: `yolo::tests`).
