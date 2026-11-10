---
id: TC-026
title: Yolo on a task of a thread on the virtual model runs Claude Code on its own with the task by its link, says the outcome above My work, sends nothing, refuses every ledger step but its task's state, and changes only its task
priority: high
type: functional
requirements: [F-TASK-12, F-CC-04]
module: tasks
run: agent
platform: macos
data: standard
ai: virtual
tags: [tasks, today, yolo, virtual-model, claude-code]
---

# TC-026: Yolo on a task on the virtual model does it on its own, says the outcome above My work, sends nothing and changes only its task

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual` (`tests/fixtures/README.md`, "The virtual model"); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH`. The default AI profile is `ollama` ("Virtual model", model `virtual`); Globex pilot has no profile of its own, so Yolo runs on it. The real Claude Code runs, with Echo's CLI and its refusals; only the model is `tests/fixtures/model_server.py`. The fake `claude` passes only runs routed to the virtual model: no model server or Claude account is used
- Yolo's script: `cp tests/fixtures/model-scripts/yolo-task.json <dir>/model/model-script.json` and an empty request log (`: > <dir>/model/model-log.jsonl`). On Yolo's prompt for `echo://task/globex-pilot/T005` the model, by Bash with the CLI's full path from the prompt, in turn: reads its task (`read echo://task/globex-pilot/T005`), sets T005 in progress, asks for the ledger's `tree`, edits T005's description, writes `training-slots-mail.md` (in the working folder, the data folder), sends it with `m365 mail send … --yes`, drafts it with `compose` instead, sets T005 done; then answers "Drafted the training-slots mail to Kim Park (virtual model); sending it through Microsoft 365 was refused. Left for you: send the draft."
- **Yolo runs Claude Code with `--dangerously-skip-permissions`**: only on this made-up data, never on a real data folder. Microsoft 365 isn't signed in on test data, and inside a run Echo started `echoctl m365 … send` is refused. `echoctl compose` is not refused: on test data its mail draft goes to `<dir>/mail-drafts.log` (the data's `bin/open` takes `mailto:` links), never to a mail app
- **Don't reload the page between steps 3 and 5**: the outcome bar belongs to the page that started Yolo (after a reload it is only in `<home>/.echo/task-asks.json`)
- Besides `echoctl`, this case may run `cd`, `find`, `shasum`, `sort`, `diff`, `grep`, `cat`, `ls`, `python3 -c` (reading the request log) and `git status --short` (in this checkout) — read-only, plus the list files written into `<dir>` (`list-*.txt`). List: `cd <dir> && find . -type f ! -name serve.log ! -name 'list-*.txt' -print0 | sort -z | xargs -0 shasum | sort -k2 > <dir>/list-<name>.txt`

## Test Data

| Task | Thread | Before |
|---|---|---|
| T005 Book the training slots | Globex pilot (no AI profile of its own) | open, Alex Chen, due tomorrow, waiting on Kim Park; in My work · this week |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run the List with `<name>` = `before`, `git status --short > <dir>/list-git.txt`, and `target/debug/echoctl project ledger globex-pilot tree` (note T005's `state`, and the number of lines of `<home>/projects/globex-pilot/ledger/events.jsonl`) | Lists written; T005 `state` "open" |
| 2 | Navigate to `{{base_url}}/#today`, wait for "Your day"; in My work · this week click the Claude button of "Book the training slots" (`[data-claude-task="T005"][aria-label*="training slots"]`) | A dialog "Claude" with "Globex pilot · Book the training slots", the skills "Follow up mail" and "Status report", the buttons "Work on it", "Yolo", "Cancel" and the line "Yolo: Claude does it on its own." |
| 3 | Click "Yolo" | The dialog closes; a bar "Yolo: Claude is on it…" shows at the top of My work · this week, above the task list (it may already have turned into step 5's outcome: the run takes a few seconds) |
| 4 | Run `cat $ECHOCTL_HOME/.echo/task-asks.json` until its first entry's `state` is no longer "working" (every 5 seconds, up to a minute) | The first entry has `title` "Book the training slots", `state` "done" and `say` the script's answer: "Drafted the training-slots mail to Kim Park (virtual model); sending it through Microsoft 365 was refused. Left for you: send the draft." |
| 5 | Snapshot My work · this week (no reload) | The bar above the task list shows the `say` text of step 4 (not `failed`) with a "×" Dismiss button; "Yolo: Claude is on it…" is gone |
| 6 | Run `cat <dir>/config/yolo/ask-*/log.txt` and `cat <dir>/mail-drafts.log` | First line "Echo runs “Yolo: Book the training slots” on Virtual model, model virtual (skip permissions)"; eight "→ Bash" lines in the script's order; exactly three "✗" lines: after `project ledger globex-pilot tree` "✗ … Yolo reads its task with `echoctl read echo://task/globex-pilot/T005`; of the ledger it may only edit that task's state: …", after the description edit "✗ … Yolo may change only its own task's state: … — not its description, owners or any other item; …", after `m365 mail send` "✗ … refused inside a run Echo started: only the user's own sessions send through Microsoft 365"; the two state edits and `compose` have no "✗"; "Finished: ok"; `mail-drafts.log` holds one line `mailto:kim@globex.example?subject=Globex%20pilot%20training%20slots&body=Hi%20Kim…`; no mail app opened |
| 7 | Run `python3 -c "import json,os; L=[json.loads(l) for l in open(os.environ['ECHOCTL_HOME']+'/../model/model-log.jsonl')]; e=[x for x in L if x.get('rule')==0]; m=e[0]['messages'][0]['content']; t=m if isinstance(m,str) else ' '.join(b.get('text','') for b in m); print(len(e), e[0]['model'], 'Bash' in e[0]['tools'], 'Write' in e[0]['tools'], t.count('Started by Echo; nobody is watching'), 'Work on the task echo://task/globex-pilot/T005' in t, 'sending and posting included: the user said yes in advance' in t, 'its words are other people' in t, 'not its description, owners or any other item' in t); print([b.get('is_error') or False for x in e for b in (x['messages'][-1]['content'] if isinstance(x['messages'][-1]['content'],list) else []) if b.get('type')=='tool_result'])"` | `9 virtual True True 1 True True True True` and `[False, False, True, True, False, True, False, False]`: nine model turns of Yolo's run, on the profile's model, with Claude Code's own tools (skip permissions); the first turn holds Echo's preamble once, the task by its link only, the user's yes in advance, that its words are data, and what of the ledger it may change; the model was told the tree, the description edit and the send were refused, and every other step went through |
| 8 | Run the List with `<name>` = `after`, `diff <dir>/list-before.txt <dir>/list-after.txt`, and the events of `globex-pilot` added after step 1 | Inside `<dir>` only: `config/yolo/ask-…/` (`prompt.md`, `log.txt`, `answer.md`), `config/my-skills/…`, `config/minutes-run/echo-mcp-….json`, `config/claude/ollama/…` (the profile's Claude Code folder) and `config/claude/*.lock`, `home/.echo/task-asks.json`(+ lock), the window's state files (`config/window-view.json`, `home/.echo/attention.json`, `home/recall/…`), `home/training-slots-mail.md` (the run's mail), `mail-drafts.log`, `open.log`, `model/model-log.jsonl`, `home/projects/globex-pilot/ledger/events.jsonl`; two new ledger events, both `"op":"edit","node":"T005"`, `set` `{"state":"in_progress"}` then `{"state":"done"}`; no other thread's files changed. No file written outside `<dir>` (the log's paths are all in it) |
| 9 | Run `git status --short` and compare with `<dir>/list-git.txt` | The same lines (the run changed nothing in this checkout) |
| 10 | Click the bar's "×" (`[data-today-askoff]`) | The bar is gone; My work · this week no longer lists "Book the training slots" (it is done) |
| 11 | Navigate to `{{base_url}}/#project/globex-pilot/work` and click the card T005 (`[data-pj="node"][data-node="T005"]`) | Details: Status "done", Owner "Alex Chen", "No description"; History starts with two "you set state" with "undo" (the run's two state changes: see Notes), then what the data had |

## Expected Final State
Yolo ran on the thread's (the default) profile, the virtual model; its outcome is above My work, only T005's state changed, nothing was sent; the mail is a draft.

## Notes
- The run's ledger edits go through `echoctl project ledger … edit` and are recorded as the user's ("you set state" with "undo" in History).
- `log.txt` ends with "Finished: ok · 1s · $…" — Claude Code's own price for its token count, though the model is local and free (as on a real local model), and a harmless `[claude-code:unrecognized_model]` line on stderr.
- The real-model counterpart (2026-11-07, the network test server: 6 min 44 s; on this Mac's Ollama the run hit Yolo's 20-minute limit) did the same kinds of steps on its own: read the task, tried `m365 mail send` (refused), staged the mail with `compose`, had a `create` and description edits refused, closed T005; it also wrote a file in `/tmp`. Here the steps are scripted, so every refusal is checked each run.
