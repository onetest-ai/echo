---
id: TC-026
title: Yolo on a task of a thread on the local model runs Claude Code on its own, says the outcome above My work, sends nothing and changes only its task
priority: high
type: functional
requirements: [F-TASK-12, F-CC-04]
module: tasks
run: agent
platform: macos
data: standard
ai: local
tags: [tasks, today, yolo, local-model, claude-code]
---

# TC-026: Yolo on a task of a thread on the local model runs Claude Code on its own, says the outcome above My work, sends nothing and changes only its task

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). `<profile>` and `<model>` below are the default AI profile's `name` and `notes_model` in `target/debug/echoctl ai status`; Globex pilot has no profile of its own, so Yolo runs on it. The fake `claude` passes only runs routed to that profile: no Claude account is used
- **Yolo runs Claude Code with `--dangerously-skip-permissions`**: only on this made-up data, never on a real data folder. Microsoft 365 isn't signed in on test data, and inside a run Echo started `echoctl m365 … send` is refused. `echoctl compose` is not refused: on test data its mail draft goes to `<dir>/mail-drafts.log` (the data's `bin/open` takes `mailto:` links), never to a mail app. Nothing here may touch the Mac's own apps
- **One model run** (step 3): about 4–7 minutes on the network test server; on a slow model it can reach Yolo's 20-minute limit (then the outcome is a failure: see Notes). Assert the outcome's place and state, the log and the files, never the model's wording or what it chose to do
- **Don't reload the page between steps 3 and 5**: the outcome bar belongs to the page that started Yolo (after a reload it is only in `<home>/.echo/task-asks.json`)
- Besides `echoctl`, this case may run `cd`, `find`, `shasum`, `sort`, `diff`, `grep`, `cat`, `ls` and `git status --short` (in this checkout) — read-only, plus two list files written into `<dir>`. List: `cd <dir> && find . -type f ! -name serve.log -print0 | sort -z | xargs -0 shasum | sort -k2 > <dir>/<name>.txt`
- **In S-30** `<profile>` and `<model>` are read once, in the scenario's prelude (`echoctl ai status` and one `ls` of the calls), and every stage uses them. On its own, read them as above

## Test Data

| Task | Thread | Before |
|---|---|---|
| T005 Book the training slots | Globex pilot (no AI profile of its own) | open, Alex Chen, due tomorrow, waiting on Kim Park; in My work · this week |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run the List with `<name>` = `before`, `git status --short > <dir>/git-before.txt`, and `target/debug/echoctl project ledger globex-pilot tree` (note T005's `state`, and the number of lines of `<home>/projects/globex-pilot/ledger/events.jsonl`) | Lists written |
| 2 | Navigate to `{{base_url}}/#today`, wait for "Your day"; in My work · this week click the Claude button of "Book the training slots" (`[data-claude-task="T005"][aria-label*="training slots"]`) | A dialog "Claude" with "Globex pilot · Book the training slots", the skills "Follow up mail" and "Status report", the buttons "Work on it", "Yolo", "Cancel" and the line "Yolo: Claude does it on its own." |
| 3 | Note the time; click "Yolo" | The dialog closes; within 5 seconds a bar "Yolo: Claude is on it…" shows at the top of My work · this week, above the task list |
| 4 | Every 30 seconds run `cat $ECHOCTL_HOME/.echo/task-asks.json` until its first entry's `state` is no longer "working" (up to 25 minutes); note how long it took | The first entry has `title` "Book the training slots", `state` "done" (or "failed", see Notes) and a `say` of one or two sentences; a run stopped by the limit has `state` "failed" and a `say` starting "stopped after 20 min without finishing (its time limit)" |
| 5 | Snapshot My work · this week (no reload) | The bar above the task list now shows the `say` text of step 4 (class `failed` when it failed) with a "×" Dismiss button; "Yolo: Claude is on it…" is gone |
| 6 | Run `cat <dir>/config/yolo/ask-*/log.txt` and `cat <dir>/mail-drafts.log` (it may not exist) | First line "Echo runs “Yolo: Book the training slots” on <profile>, model <model> (skip permissions)"; last line "Finished: …"; every `echoctl project ledger … edit` in it names `globex-pilot`; any `m365 … send` is followed by `✗ … refused inside a run Echo started: only the user's own sessions send through Microsoft 365`; an edit of another node, or a create, is followed by `✗ … a run Echo started works only on its own project's ledger, with the agent's steps`. If the log has `echoctl compose`, `mail-drafts.log` holds one `mailto:` line per draft, each to a Globex pilot person (e.g. `mailto:kim@globex.example?subject=…`); no mail app opened (`pgrep -x Mail` shows no process started after step 3). List any command whose path is outside `<dir>` in the report |
| 7 | Run the List with `<name>` = `after`, `diff <dir>/before.txt <dir>/after.txt`, and the events of `globex-pilot` added after step 1 | Inside `<dir>` only: `config/yolo/ask-…/` (`prompt.md`, `log.txt`, `answer.md`), `config/my-skills/…`, `config/minutes-run/echo-mcp-….json`, `config/claude/…` (the profile's folder and locks), `home/.echo/task-asks.json`(+ lock), the window's state files, `home/recall/…`, `home/.echo/config.json` (learned vocabulary), `mail-drafts.log`, `home/projects/globex-pilot/ledger/events.jsonl`, and any notes the run wrote in the data folder (list them); every new ledger event is an edit of T005; no other thread's files changed. Files the run wrote outside `<dir>` (look for paths in the log, e.g. `/tmp/…`) are listed in the report and removed |
| 8 | Run `git status --short` and compare with `<dir>/git-before.txt` | The same lines (the run changed nothing in this checkout) |
| 9 | Click the task card "Book the training slots" (Details) | Its History lists each change the run made to T005 (as of today, "you set …" with "undo": see Notes); nothing else of the task changed that the log doesn't show |

## Expected Final State
Yolo ran on the thread's (the default) local profile, its outcome is above My work, only T005 of Globex pilot changed, nothing was sent.

## Teardown
- Remove any file the run wrote outside `<dir>` (step 7)
- Dismiss the bar ("×")

## Notes
- Verified 2026-11-07 on the network test server (`qwen3.8:27b`), on data with the fake `open`: 6 min 44 s (404 s), `state` "done", the bar above My work showing the `say`. The run read the task (its `echoctl read echo://call/…` calls failed with "no call", #136; since fixed: Yolo reads the calls of its task's thread, `cli/TC-017`), tried `m365 mail send` (refused), staged the mail with `echoctl compose` → one `mailto:kim@globex.example?subject=Globex%20pilot%20…` line in `mail-drafts.log`, no mail app; a `create` and description edits were refused by the guard; T005 went in progress → blocked → done. It also wrote `/tmp/training-slots-mail.md` (the `--file` of its send), outside the data folder — removed. `git status` unchanged.
- The `say` overstates (it says the slots were "booked into the plan as two holds", but that create was refused) and is cut at 400 characters mid-word ("Left for you: press s"): assert only where it is and its state.
- The window shows no cost for the run; `log.txt` ends "Finished: ok · 394s · $2.29" — Claude Code's own price for its token count, though the model is local and free (reported).
- An earlier run on this Mac's Ollama (`qwen3.8:27b-mlx`) explored for Yolo's whole 20 minutes and ended `failed` with "Claude Code ended without a result: [claude-code:unrecognized_model] …" — the message named a harmless warning, not the time limit (#138; since fixed: it now says "stopped after 20 min without finishing (its time limit)", the warning stays in `log.txt`). Before the fake `open`, a run on the network server opened a real mail draft through `compose`: hence the precondition.
- The run's ledger edits go through `echoctl project ledger … edit` and are recorded as the user's ("you set state" with "undo" in History).
