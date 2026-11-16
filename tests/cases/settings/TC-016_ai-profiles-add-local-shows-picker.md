---
id: TC-016
title: A local model profile added from the CLI shows in AI & minutes and turns the account chip into a picker
priority: high
type: functional
requirements: [F-AI-01, F-AI-02, F-AI-07, F-AI-09]
module: settings
run: agent
platform: any
tags: [ai, profiles]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-016: A local model profile added from the CLI shows in AI & minutes and turns the account chip into a picker

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** the AI profile Local Llama (`local-llama`), not the default. In S-12: added by stage 8 (settings/TC-027 step 4; its teardown only makes `system` the default again), so step 1 is skipped there; this case's teardown is skipped too, for stage 10. On its own: step 1 adds it.

**Changes:** `<config dir>/ai.json` (the AI profiles)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash run `echoctl ai add-local local-llama --model llama3.2 --url http://127.0.0.1:9 --name "Local Llama"` | Prints `added Local Llama (local-llama)` |
| 2 | Navigate to `{{base_url}}/#settings/ai`; read the card "Profiles" | Two rows: "Claude Code (as configured)" still "Default for new calls", and "Local Llama" with the tag "Model · this computer", the text "minutes: llama3.2" and "live notes: llama3.2", and a button "Make default" |
| 3 | Read the selects in the top bar (`#r-top select`) | "Language of the call" in the Record pill and "AI profile" in the account chip at the right end (options "Claude Code (as configured)", selected, and "Local Llama"); the Record pill holds no profile picker |
| 4 | In Bash run `echoctl ai status` | Two profiles; `local-llama` has kind `local`, `"default": false` |

## Expected Final State
Two profiles are listed and the account chip picks between them.

## Teardown
- `echoctl ai remove local-llama` (prints `removed local-llama`).

## Notes
- The profile's address is `http://127.0.0.1:9`, where nothing listens: `echoctl ai status` sends a local profile's server a request, and the Ollama on this computer (`localhost:11434`) is shared, never a test's.
- No model server is needed: the profile is only added, never used.
