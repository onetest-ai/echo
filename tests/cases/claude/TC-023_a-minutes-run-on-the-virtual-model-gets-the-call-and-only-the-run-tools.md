---
id: TC-023
title: A minutes run on the virtual model gets the call's process-call prompt and only the run's tools, and the minutes and action items it writes show in the window
priority: high
type: functional
requirements: [F-MIN-13, F-CC-01]
module: claude
run: agent
platform: macos
data: small
ai: virtual
tags: [claude, minutes, virtual-model]
---

# TC-023: A minutes run on the virtual model gets the call's prompt and only the run's tools; its minutes show

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Small data with the virtual model: `tests/fixtures/serve.sh up <dir> <port> --ai virtual` (`tests/fixtures/README.md`, "The virtual model"): the real Claude Code and Echo's guard run, only the model is `tests/fixtures/model_server.py`, answering from its default script (`<dir>/model/model-script.json`) and logging every request to `<dir>/model/model-log.jsonl`. Claude Code on `PATH`. The window is served at `{{base_url}}`; `{{base_url}}/` was loaded first (the cookie)
- Bash steps run after `. <dir>/env`

## Test Data

| Field | Value |
|---|---|
| Call | Initech planning (`*-initech-planning`), transcribed, no minutes |
| The script's minutes | "The virtual model wrote these minutes from the transcript it read."; action item "Send the follow-up notes" (Me) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click `[role=option][data-call$="initech-planning"]`, then the tab "Minutes" | "No minutes yet", "Claude Code writes them with Virtual model (virtual), on this computer." and "Write minutes" |
| 2 | Click "Write minutes" (`[data-cmd="write-notes"]`) and wait up to 30 s | The minutes show: the notice "Written by AI (Echo, Virtual model · virtual · Discussion template)…", Summary "The virtual model wrote these minutes from the transcript it read.", Action items "Me — send the follow-up notes" |
| 3 | Run `python3 -c "import json,os; L=[json.loads(l) for l in open(os.environ['ECHOCTL_HOME']+'/../model/model-log.jsonl')]; L=[e for e in L if e['path']=='/v1/messages']; print(len(L), L[0]['model'], sorted(set(L[0]['tools'])-{'DeferredToolPlaceholder'}))"` | `6 virtual ['Bash', 'Edit', 'Glob', 'Grep', 'Read', 'Skill', 'Write']`: six model turns, and the run's tools only (no web, no agents, no MCP) |
| 4 | Run `python3 -c "import json,os; e=json.loads(open(os.environ['ECHOCTL_HOME']+'/../model/model-log.jsonl').readline()); m=e['messages'][0]['content']; t=m if isinstance(m,str) else ' '.join(b.get('text','') for b in m); print('<command-args>'+os.environ['ECHOCTL_HOME'] in t, 'write the new action list to' in t, 'nobody is watching it' in t)"` | `True True True`: the first turn names this call's folder, the run's own action file and that nobody is watching |
| 5 | Read the call's `claude.log` (`$ECHOCTL_HOME/calls/*-initech-planning/claude.log`) | "→ Bash echoctl context …", "→ Read …/transcript.md", two "→ Write" (notes.md and `actions.new-<run>.json`), "→ Bash echoctl actions … --apply …", no "✗", and "Finished: ok" |
| 6 | Read the call's `actions.json` and `notes-run.json` | One item "Send the follow-up notes", owner "Me", status "todo", with an id; the run's `state` is "done" |

## Expected Final State
Initech planning has minutes and one action item, written through the real Claude Code, its tools and Echo's guard.

## Notes
- No real model and no Claude account: the whole run takes a couple of seconds and repeats exactly. A case that needs other answers writes its own script before its run (`tests/fixtures/model-scripts/`, e.g. `hostile/TC-019`).
