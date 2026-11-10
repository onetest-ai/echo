---
id: S-32
title: "Model runs on the virtual model: what Echo gives the model, and what the guard lets it do"
run: agent
data: small
ai: virtual
cases:
  - tests/cases/claude/TC-023_a-minutes-run-on-the-virtual-model-gets-the-call-and-only-the-run-tools.md
  - tests/cases/hostile/TC-019_a-model-that-tries-to-leave-the-call-is-refused-by-the-guard.md
auto: [tests/auto/specs/S-32.spec.mjs]
---

# S-32: Model runs on the virtual model

**Story:** Sam writes a call's minutes, and a transcript talks a model into trying to leave its call. Claude Code, its tools, Echo's guard and the app run for real; the model is the virtual one (`tests/fixtures/model_server.py`), answering from a script and logging what it was given.

**Data:** `small` with the virtual model (`tests/fixtures/serve.sh up <dir> <port> --ai virtual`; `tests/fixtures/README.md`, "The virtual model"). Fresh data, built right before the run. Needs Claude Code on `PATH`; no model server, no Claude account.

**Length:** a few minutes; each model run takes a second or two.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`small --ai virtual`) and its virtual model | the lead (`serve.sh up … --ai virtual`), right before the run | all |
| The default script (a minutes run) | the build | 1 |
| The guard-probe script | stage 2's Preconditions (`guard-probe.json` with the other call's folder filled in) | 2 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `claude/TC-023_a-minutes-run-on-the-virtual-model-gets-the-call-and-only-the-run-tools.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `hostile/TC-019_a-model-that-tries-to-leave-the-call-is-refused-by-the-guard.md` | — (another call) | Write the guard-probe script (its Preconditions); clear `<dir>/model/model-log.jsonl` (`: > …`) so its step 4 reads only this run |

## Teardown
- `tests/fixtures/serve.sh down <dir>` (it stops the virtual model too)

## Notes
- The real-model counterparts stay for releases: S-31 (a live model on hostile words) and S-30.
- The prep, Yolo and the thread's sync run on the virtual model in S-33: they need the `standard` data (a thread's meetings, tasks and ledger), and a scenario has one data build.
- More `small`-data model cases move here as their scripts are written (#275): the rest of S-19, live notes, diagram repair.
