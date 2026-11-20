---
id: TC-004
title: A meeting's prep on the virtual model leaves a trace of its model call — what it was given, what it answered, its tokens
priority: high
type: functional
requirements: [F-TRACE-05]
module: traces
run: agent
platform: macos
data: standard
ai: virtual
tags: [traces, telemetry, prep, virtual-model]
---

# TC-004: A prep on the virtual model leaves a trace of its model call

**Module:** Traces | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env` (abort if `ECHOCTL_HOME` isn't inside `<dir>`)
- Acme renewal follow-up's prep was written on this data by `echoctl prep --write` (`prep/TC-015`, its stage in S-33): one request to the virtual model's Messages API, no Claude Code
- Besides `echoctl`, this case may run `python3 -c` reading the command's output

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl traces --kind model` | One trace: `kind` "model", `name` "model.request", `ok` true, `tokens` a number above 0, `model` "virtual". Note its `id` |
| 2 | Run `echoctl traces show <id>` | One span "model.request" with `attrs`: `profile` "ollama", `model` "virtual", `ok` true; `system` starting "You prepare a busy person for a meeting"; `input` naming "Acme renewal follow-up"; `output` holding the script's answer (its `in_short`); `usage` with `input_tokens` and `output_tokens` |
| 3 | Run `echoctl traces --kind job` | The prep's call isn't listed there (a prep written from the CLI runs in no job) |

## Expected Final State
Nothing changes.

## Teardown
- None (the scenario's teardown stops the server)

## Notes
- Requests to Microsoft 365 (method, URL, never a header), a look that found nothing leaving nothing unless a step failed, and tokens counted in a job's trace: Rust tests (`telemetry::tests`, `graph::tests::a_request_to_microsoft_is_traced_…`). A transcription's span: read in the code (`transcribe::transcribe`); its cases record calls.
