---
id: TC-018
title: echoctl doctor loads a local profile's model and warns when its Ollama server gives it a small context; ai status warns once it is loaded; a server that isn't Ollama gets no warning
priority: high
type: functional
requirements: [F-AI-10]
module: cli
run: agent
platform: any
data: small
tags: [cli, ai, profiles, ollama, context]
---

# TC-018: echoctl doctor loads a local profile's model and warns when its Ollama server gives it a small context; ai status warns once it is loaded; a server that isn't Ollama gets no warning

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Small data set built right before the run: `tests/fixtures/serve.sh up <dir> <port>` (`data: small`); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Three stand-in Ollama servers (`tests/fixtures/fake-ollama.py`), each logging its requests to a file in `<dir>`. Start them in the background, one per port, with `L=<dir>`:
  - `python3 tests/fixtures/fake-ollama.py 48151 --ctx 4096 --log $L/small.log &` (an Ollama server that gives `small-model` 4096 tokens, once loaded)
  - `python3 tests/fixtures/fake-ollama.py 48152 --ctx 262144 --log $L/large.log &` (262144 tokens)
  - `python3 tests/fixtures/fake-ollama.py 48153 --not-ollama --log $L/other.log &` (no `/api/ps`: a server with only Anthropic's Messages API)
- No model runs; nothing reaches a real model server

**Changes:** `<config dir>/ai.json` (four AI profiles added, then removed)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl ai add-local tiny --model small-model --url http://127.0.0.1:48151 --name Tiny`, then the same with `big`, `48152`, `Big` and with `other`, `48153`, `Other`; then `echoctl ai add-local gone --model not-pulled --url http://127.0.0.1:48151 --name Gone` | Each prints `added …` |
| 2 | Run `echoctl ai status tiny` | `tiny` has no `warnings` and its `context` is empty: its model isn't loaded, and status doesn't load it. `small.log` has only `GET /api/ps` (no `POST /api/generate`) |
| 3 | Run `echoctl doctor` | An "AI profiles" part, its heading ("loading each local model…") printed before the profiles. Tiny: a warning that `http://127.0.0.1:48151` gives `small-model` only 4,096 tokens of context, that long calls' minutes will miss parts, and to set `OLLAMA_CONTEXT_LENGTH` on that server. Big: `small-model` has 262,144 tokens of context, no warning. Other: no warning; its context is unknown ("not an Ollama server, or not answering"). Gone: ⚠, "context unknown" and that the server has no model `not-pulled`, or couldn't load it (HTTP 404) — not that it isn't Ollama |
| 4 | Read the three logs | `small.log` and `large.log` each have `POST /api/generate` (the model was loaded, with `keep_alive` 30s) between two `GET /api/ps`. `other.log` has only `GET /api/ps`: no `POST` was sent to the server that isn't Ollama |
| 5 | Run `echoctl ai status tiny`, then `echoctl ai status big` | `tiny`: `context` `{"small-model": 4096}` and one entry in `warnings` naming `OLLAMA_CONTEXT_LENGTH`. `big`: `context` `{"small-model": 262144}`, no `warnings` |
| 6 | Run `echoctl doctor --json` | Its `ai` part lists the four profiles with the same `context`, `warnings` and `problems` as step 3 |
| 7 | Note the lines in `small.log`; run `echoctl doctor --request` | No "AI profiles" part; `small.log` has no new line (the repeated permission check loads nothing) |

## Expected Final State
Only the profile whose server gives its model a small context is warned about; nothing was sent to the server that isn't Ollama except one `GET /api/ps`.

## Teardown
- `echoctl ai remove tiny`, `echoctl ai remove big`, `echoctl ai remove other`, `echoctl ai remove gone`
- Stop the three `fake-ollama.py` processes this run started (by their exact command lines)

## Notes
- The floor is 64k tokens (65,536). A real Ollama gives a model the context it was started with: `OLLAMA_CONTEXT_LENGTH` when set, otherwise a default from the machine's memory.
