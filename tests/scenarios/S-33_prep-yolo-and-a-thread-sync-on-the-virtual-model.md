---
id: S-33
title: "Prep, Yolo and a thread's sync on the virtual model: what Echo gives the model, what it lets it do, and what it does with the answer"
run: agent
data: standard
ai: virtual
cases:
  - tests/cases/prep/TC-015_prep-write-on-the-virtual-model-writes-its-summary.md
  - tests/cases/tasks/TC-026_yolo-on-the-virtual-model-does-the-task-and-says-so-above-it.md
  - tests/cases/work/TC-028_sync-on-the-virtual-model-reads-a-call-and-keeps-what-you-set.md
  - tests/cases/tasks/TC-031_yolo-preparing-and-asking-drafts-instead-of-sending.md
  - tests/cases/tasks/TC-034_yolo-runs-shows-a-prepared-draft-not-a-send.md
  - tests/cases/tasks/TC-032_yolo-runs-lists-every-run-with-what-it-did.md
  - tests/cases/tasks/TC-033_yolo-runs-filters.md
auto: [tests/auto/specs/S-33.spec.mjs]
---

# S-33: Prep, Yolo and a thread's sync on the virtual model

**Story:** Sam has a meeting's prep written, gives a task to Claude with Yolo — acting on its own, then with the choice off, preparing and asking —, and syncs Acme rollout's work after editing two of its items himself; then he hands Claude two more tasks and a mail, and looks back over every Yolo run on Yolo runs. Claude Code, its tools, Echo's guard, the CLI's refusals and the ledger run for real; the model is the virtual one (`tests/fixtures/model_server.py`), answering each job from its own script and logging what it was given.

**Data:** `standard` with the virtual model (`tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; `tests/fixtures/README.md`, "The virtual model"). Fresh data, built right before the run, as `make_data.py` allows (06:00–22:50; the prep's meeting starts about 40 minutes after the build). Needs Claude Code on `PATH`; no model server, no Claude account. (The `small`-data model runs are S-32: one data set per scenario.)

**Length:** a few minutes; each model job takes a second or two.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`standard --ai virtual`) and its virtual model | the lead (`serve.sh up … --set standard --ai virtual`), right before the run | all |
| The prep script (`prep-summary.json`) and model job 1: Acme renewal follow-up's prep (one request, no Claude Code) | stage 1's Preconditions and step 2 | 1 |
| The Yolo script (`yolo-task.json`) and model run 2: Yolo on Globex pilot's T005 | stage 2's Preconditions and step 3 | 2 (bar, task-asks, log, drafts, file diff, git, History) |
| The sync script (`thread-sync.json`) and model run 3: Acme rollout's sync (extract, verify, people) | stage 3's Preconditions and step 3 | 3 (status, run folder, tree, pinned fields, log, people, window) |
| The asks script (`yolo-asks.json`) and model run 4: Yolo on Globex pilot's T009 with "Claude acts on its own" off | stage 4's Preconditions and step 1 | 4 (model log, drafts, record, ledger), 5 (its row on Yolo runs) |
| The runs script (`yolo-runs.json`) and model runs 5–7: Yolo on Acme rollout's T010, Globex pilot's T001 and the Needs you now item "Mail to you" | stage 6's step 1 | 6 (the list, task link, log, ⌘K), 7 (filters) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `prep/TC-015_prep-write-on-the-virtual-model-writes-its-summary.md` | — | Load `{{base_url}}/` (the cookie). Write the prep script and clear the log (its Preconditions). Steps 4–5 in one shell call |
| 2 | `tasks/TC-026_yolo-on-the-virtual-model-does-the-task-and-says-so-above-it.md` | — (another thread, another job) | Write the Yolo script; clear `<dir>/model/model-log.jsonl` (`: > …`) so its step 7 reads only this run. Step 1's commands in one shell call |
| 3 | `work/TC-028_sync-on-the-virtual-model-reads-a-call-and-keeps-what-you-set.md` | — (another thread) | Write the sync script; clear `<dir>/model/model-log.jsonl` so its step 9 reads only this sync. Step 1 and step 2's first `pending` in one shell call; step 2's two `done` (the ids from that `pending`), its second `pending` and the saved tree in a second |
| 4 | `tasks/TC-031_yolo-preparing-and-asking-drafts-instead-of-sending.md` | 2 (its teardown switched the choice back off) | Write the asks script; clear `<dir>/model/model-log.jsonl` |
| 5 | `tasks/TC-034_yolo-runs-shows-a-prepared-draft-not-a-send.md` | 4 (its run) | — |
| 6 | `tasks/TC-032_yolo-runs-lists-every-run-with-what-it-did.md` | — (its own runs; the list also holds stages 2 and 4's) | Write the runs script; clear `<dir>/model/model-log.jsonl` |
| 7 | `tasks/TC-033_yolo-runs-filters.md` | 6 (its runs) | — |

## Teardown
- `tests/fixtures/serve.sh down <dir>` (it stops the virtual model too)

## Notes
- Each stage writes its own script into `<dir>/model/model-script.json` (read on every request), so no stage's model answers another's prompt; each script's rules are keyed on phrases only its job's prompt holds.
- Stage 2 runs with Yolo's skip permissions: only on this made-up data. The stages touch different threads (Acme renewal's prep, Globex pilot's T005, Acme rollout's work), so none depends on another; run them one after another, never side by side: stage 2 diffs the whole data folder around its run.
- The real-model counterparts of these three ran in S-30 on `--ai local` (minutes of model time each, 45 min for the sync on this Mac); they moved here, where every scripted step — the guard's and the CLI's refusals included — is checked on every run.
