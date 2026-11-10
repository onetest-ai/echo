---
id: TC-017
title: Make default moves "Default for new calls" and the account chip follows
priority: high
type: functional
requirements: [F-AI-01, F-WS-07]
module: settings
run: agent
platform: any
tags: [ai, profiles]
---

# TC-017: Make default moves "Default for new calls" and the account chip follows

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** the AI profile Local Llama (`local-llama`), not the default. In S-12: added by stage 8 (settings/TC-027) and kept by stage 9, so step 1 is skipped there. On its own: step 1 adds it.

**Changes:** `<config dir>/ai.json` key `default`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash run `echoctl ai add-local local-llama --model llama3.2 --url http://localhost:11434 --name "Local Llama"` | Prints `added Local Llama (local-llama)` |
| 2 | Navigate to `{{base_url}}/#settings/ai` and click "Make default" on the row "Local Llama" | "Local Llama" now has the tag "Default for new calls"; "Claude Code (as configured)" has a "Make default" button |
| 3 | Read the account chip at the top bar's right end | "Local Llama" |
| 4 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/ai.json"` | The file has `"default": "local-llama"` |

## Expected Final State
Local Llama is the default profile in the page, the account chip and `ai.json`.

## Teardown
- `echoctl ai default system` (prints `new calls use Claude Code (as configured)`), then `echoctl ai remove local-llama`.
