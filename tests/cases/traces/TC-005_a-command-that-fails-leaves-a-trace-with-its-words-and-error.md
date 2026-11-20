---
id: TC-005
title: A CLI command that fails leaves a trace of its own with its command and its error, never its arguments
priority: medium
type: functional
requirements: [F-TRACE-06]
module: traces
run: agent
platform: any
data: standard
tags: [traces, telemetry, cli]
---

# TC-005: A command that fails leaves a trace with its command and its error, never its arguments

**Module:** Traces | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set (any), built right before the run; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env` (abort if `ECHOCTL_HOME` isn't inside `<dir>`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl traces show 0123` | Exit not 0: "“0123” isn't a trace id" |
| 2 | Run `echoctl traces --kind command` | Its first trace: `kind` "command", `name` "cli.traces", `ok` false, `error` "“0123” isn't a trace id (32 hex digits, as `echoctl traces` lists them)" |
| 3 | Run `echoctl traces show <its id>` | One span "cli.traces" whose `attrs.command` is "traces show" and `attrs.error` the same words; no `args` |
| 4 | Run `echoctl server use http://127.0.0.1:9 --key sk-made-up-0000`, then `echoctl traces --kind command --limit 1` and `echoctl traces show <its id>` | The first fails (the server can't be reached); its trace: `name` "cli.server", `attrs.command` "server use", `ok` false, `attrs.error` why it failed; "sk-made-up-0000" appears nowhere in `traces show`'s output |

## Expected Final State
Nothing changes.

## Teardown
- None
