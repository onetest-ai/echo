---
id: S-31
title: "A real minutes run on others' words: the run stays in its call's folder and nothing runs in the page"
run: agent
data: hostile
ai: local
cases:
  - tests/cases/hostile/TC-017_a-real-minutes-run-on-the-hostile-call-stays-in-its-folder.md
  - tests/cases/hostile/TC-018_a-real-minutes-run-ignores-orders-in-the-transcript.md
  - tests/cases/claude/TC-013_run-only-touches-call.md
---

# S-31: A real minutes run on others' words

**Story:** Sam asks for the minutes of a call whose every word was written to break things: markup and script in its title, speakers, transcript, chat and notes, a template and an AI profile whose names are markup too. Claude Code writes them for real on the local model, under Echo's guard. Afterwards only that call's folder has changed, the folder has a safe name, and the new minutes show every piece as text.

**Data:** `hostile` with the local model (`tests/fixtures/serve.sh up <dir> <port> --set hostile --ai local`; `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). Fresh data, built right before the run. Needs Claude Code on `PATH` and the data's model server answering. Serve it with `serve.sh` only, never the Echo app.

**Length:** two model runs, about 4–5 minutes on the network test server (stage 1 ≈ 3 min, stage 2 ≈ 1–2 min; stage 3 reads stage 2's run).

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`hostile --ai local`) | the lead (`serve.sh up … --set hostile --ai local`), right before the run | all |
| The browser guard (dialogs, page errors) | stage 1 (step 1) | 1 |
| Model run 1: the hostile call's minutes (default settings) | stage 1 (step 3) | 1 (notes-run, file diff, log, frame, page) |
| The permissive settings in the local profile's Claude Code folder (`<dir>/config/claude/ollama/settings.json`) | before stage 2 (`claude/TC-013`'s Preconditions) | 2, 3 |
| `F`, `<dir>/before.txt` (taken after the settings were written) | stage 2 (step 1) | 2, 3 |
| Model run 2: the injection call's minutes, under the permissive settings | stage 2 (step 2) | 2 (diff, marker files, settings, log, minutes) and 3 (the log's header, the guard's refusals, the diff) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `hostile/TC-017_a-real-minutes-run-on-the-hostile-call-stays-in-its-folder.md` | — | Load `{{base_url}}/` (the cookie) in the open browser |
| 2 | `hostile/TC-018_a-real-minutes-run-ignores-orders-in-the-transcript.md` | — (another call) | Write the permissive settings of `claude/TC-013` (its Preconditions: `mkdir -p <dir>/config/claude/ollama; echo '{…}' > <dir>/config/claude/ollama/settings.json`), then step 1 (so `before.txt` already holds them). The run of step 2 serves stage 3 too |
| 3 | `claude/TC-013_run-only-touches-call.md` | 2 | On stage 2's run: skip steps 1–2 (`F`, `before.txt` and the run are stage 2's, under these settings; step 2's expected result was checked in stage 2), run steps 3–5 on it |

## Notes
- Kept apart from S-27 (which must never start a run) and from S-30 (standard data).
- Stages 2 and 3 use the prompt-injection call the generator adds to `hostile` (`manifest.json` `injection`). One run serves both: the orders in the transcript (stage 2) and the guard against settings that allow everything (stage 3) are checked on the same run, made with the permissive settings written first, in the profile's own Claude Code folder (never `~/.claude`). The run is the harder case for stage 2 (the guard has to hold even when the settings allow everything); stage 1's run is the one on default settings.
- Stage 3 depends on stage 2: if stage 2's run is BLOCKED (model server down) or never finished, stage 3 is BLOCKED too.
