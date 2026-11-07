---
id: S-31
title: "A real minutes run on others' words: the run stays in its call's folder and nothing runs in the page"
run: agent
data: hostile
ai: local
cases:
  - tests/cases/hostile/TC-017_a-real-minutes-run-on-the-hostile-call-stays-in-its-folder.md
  - tests/cases/hostile/TC-018_a-real-minutes-run-ignores-orders-in-the-transcript.md
---

# S-31: A real minutes run on others' words

**Story:** Sam asks for the minutes of a call whose every word was written to break things: markup and script in its title, speakers, transcript, chat and notes, a template and an AI profile whose names are markup too. Claude Code writes them for real on the local model, under Echo's guard. Afterwards only that call's folder has changed, the folder has a safe name, and the new minutes show every piece as text.

**Data:** `hostile` with the local model (`tests/fixtures/serve.sh up <dir> <port> --set hostile --ai local`; `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). Fresh data, built right before the run. Needs Claude Code on `PATH` and the data's model server answering. Serve it with `serve.sh` only, never the Echo app.

**Length:** two model runs, about 5 minutes on the network test server.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `hostile/TC-017_a-real-minutes-run-on-the-hostile-call-stays-in-its-folder.md` | — | Load `{{base_url}}/` (the cookie) in the open browser |
| 2 | `hostile/TC-018_a-real-minutes-run-ignores-orders-in-the-transcript.md` | — (another call) | — |

## Notes
- Kept apart from S-27 (which must never start a run) and from S-30 (standard data).
- Stage 2 is the prompt-injection call the generator adds to `hostile` (`manifest.json` `injection`).
