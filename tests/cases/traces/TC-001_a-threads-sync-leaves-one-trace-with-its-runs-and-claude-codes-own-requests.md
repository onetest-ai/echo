---
id: TC-001
title: A thread's sync leaves one trace — the job, its two Claude Code runs with what went in and every stream event, and Claude Code's own model requests and tools under each
priority: high
type: functional
requirements: [F-TRACE-01, F-TRACE-02]
module: traces
run: agent
platform: macos
data: standard
ai: virtual
tags: [traces, telemetry, sync, virtual-model, claude-code]
---

# TC-001: A thread's sync leaves one trace with its runs and Claude Code's own requests

**Module:** Traces | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env` (abort if `ECHOCTL_HOME` isn't inside `<dir>`); Claude Code on `PATH` (only the model is the virtual one)
- Acme rollout's sync ran on this data, as in `work/TC-028` (its stage in S-33): one sync = an extract run and a verify run; nothing else synced since
- Besides `echoctl`, this case may run `python3 -c` reading the command's output

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl traces --job thread_sync` | `traces` holds one trace: `name` "job.thread_sync", `job` "thread_sync", `ok` true, `runs` 2, `tokens` a number above 0, `model` "virtual", `start` before `end`; `kept_days` 7, `dropped` 0. Note its `id` |
| 2 | Run `echoctl traces show <id>` | `spans` holds one span: `source` "echo", `name` "job.thread_sync", with two `children` named "claude.run", each `attrs.echo.telemetry` "echo", `attrs.model` "virtual", `attrs.ok` true |
| 3 | In the first "claude.run" (the extract) read `attrs` and `events` | `attrs.args` starts with "-p" and holds `--append-system-prompt-file`; `attrs.files` holds that file's text, starting "# Echo: a thread's sync — extract"; its events with `source` "echo", in order: "claude.input" (its `attrs.text` a stream-json user message naming "Acme security review"), then "claude.system.init", "claude.assistant" and "claude.user" events (the tool calls and their results, the two refused `context` calls among them), last "claude.result.success" (Claude Code's own events on the same span, `source` "claude-code", such as "managed_settings_resolved", may sit between them) |
| 4 | Under each "claude.run" read its `children` | Each has one `source` "claude-code" span "claude_code.interaction"; under it "claude_code.llm_request" spans — four under the extract's, one under the verify's — each with `attrs.model` "virtual" and `attrs.input_tokens` a number; the extract's also "claude_code.tool" spans with "claude_code.tool.blocked_on_user" children; the interaction's events hold "api_request" ones, and no event's `attrs.prompt` or `attrs.response` holds the call's words (Claude Code's own logging of text stays off: "<REDACTED>" or absent) |
| 5 | Run `echoctl traces show 0123` and `echoctl traces show 0123456789abcdef0123456789abcdef` | The first refused: "isn't a trace id"; the second refused: "no trace" |

## Expected Final State
The sync's trace stays listed; nothing else changes.

## Teardown
- None (the scenario's teardown stops the server)

## Notes
- A pass that did nothing leaves no trace, the queue never waits, 7 days' detail: Rust tests (`telemetry::tests`).
