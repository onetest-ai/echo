---
id: TC-034
title: A profile maps Claude's names (haiku, sonnet, opus) to its provider's models, and its runs ask for the mapped one
priority: high
type: functional
requirements: [F-AI-17]
module: settings
run: agent
platform: macos
data: standard
tags: [ai, profiles, models]
---

# TC-034: A profile maps Claude's names to its provider's models

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`) after `. <dir>/env`
- The thread sync's script, told the mapped name is a model it has: `cp tests/fixtures/model-scripts/thread-sync.json <dir>/model/model-script.json`; `python3 -c "import json;p='<dir>/model/model-script.json';d=json.load(open(p));d.setdefault('model',{})['names']=['virtual','virtual-sonnet'];json.dump(d,open(p,'w'))"`
- `{{base_url}}/` was loaded just before the first window step

**Changes:** `<config dir>/ai.json`, the thread `acme-rollout`'s model and ledger

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; read the profile rows | "Claude Code (as configured)" has no "Models…"; every other row has "Models: Claude's names as they are" and "Models…" |
| 2 | Run `$E ai set ollama --model sonnet=virtual-sonnet` | `Virtual model: sonnet runs virtual-sonnet`; `$E ai set ollama --model gpt=x` and `$E ai set system --model haiku=x` are refused ("haiku, sonnet or opus"; "your own Claude Code setup") |
| 3 | Reload `{{base_url}}/#settings/ai` | The Virtual model row: "Models: sonnet → virtual-sonnet" |
| 4 | Run `$E project set acme-rollout --model sonnet`; `: > <dir>/model/model-log.jsonl`; `$E project ledger acme-rollout run` | `state` "done" with a `run` id; every line of `<dir>/model/model-log.jsonl` has `"model": "virtual-sonnet"` (none `sonnet`); the run's `log.txt` says "on Virtual model, model virtual-sonnet" |
| 5 | Click "Models…" on the Virtual model row | Three fields Haiku → (empty, placeholder "haiku"), Sonnet → `virtual-sonnet`, Opus → (empty); Save, Cancel |
| 6 | Clear Sonnet, type `virtual-opus` in Opus; click Save; wait 2 s | The row: "Models: opus → virtual-opus"; `$E ai status` shows the profile `ollama` with `"models": {"opus": "virtual-opus"}` |

## Expected Final State
The Virtual model profile runs `virtual-opus` for opus; the thread acme-rollout's model is sonnet.

## Teardown
- `tests/fixtures/serve.sh down <dir>`

## Notes
- A wrapper a plugin offers with `models` (`uses.ai.wrapper.models`): its sheet line and the profile's mapping when added are the Rust tests `plugins::capabilities::…a_plugin_offers_a_wrapper…` and `ai::command::…an_offer_carries_its_models…`; a routine asking for `opus` running the mapped model is `claudejob::…a_routine_and_a_yolo_task_run_as_the_folder_session_does`.
