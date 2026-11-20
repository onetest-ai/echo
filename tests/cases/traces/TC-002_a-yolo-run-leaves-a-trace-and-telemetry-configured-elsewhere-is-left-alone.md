---
id: TC-002
title: A Yolo run leaves a trace of its own with Claude Code's requests; with telemetry set in the profile's Claude Code settings, the next run's trace has Echo's part only and says "configured elsewhere"
priority: critical
type: functional
requirements: [F-TRACE-01, F-TRACE-02, F-TRACE-03]
module: traces
run: agent
platform: macos
data: standard
ai: virtual
tags: [traces, telemetry, yolo, virtual-model, claude-code, privacy]
---

# TC-002: A Yolo run's trace, and telemetry configured elsewhere left alone

**Module:** Traces | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; `{{base_url}}/` loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env` (abort if `ECHOCTL_HOME` isn't inside `<dir>`); Claude Code on `PATH`
- The Yolo runs of `tasks/TC-032` ran on this data (its stage in S-33), and its script is still in `<dir>/model/model-script.json`
- The IPC helper and `wait_run` of `tasks/TC-032`
- The virtual model's profile keeps its Claude Code settings in `$ECHOCTL_CONFIG_DIR/claude/ollama/settings.json` (no such file in the data)
- Besides `echoctl`, this case may run `cat`, `rm` and `python3 -c` reading the command's output, and write that settings file

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl traces --job yolo` | One trace per Yolo run on this data, newest first, each `name` "job.yolo", `runs` 1, `model` "virtual"; the failed `ask-runs-4`'s has `ok` false |
| 2 | Run `echoctl traces show <the newest one's id>` | One span "job.yolo" (`attrs.title` the task's), under it one "claude.run" with `attrs.echo.telemetry` "echo" and, under that, a "claude_code.interaction" span (`source` "claude-code") with "claude_code.llm_request" children |
| 3 | Write `{"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://127.0.0.1:9"}}` to `$ECHOCTL_CONFIG_DIR/claude/ollama/settings.json`; then `ipc '{"cmd":"task_yolo","ask":"ask-trace-1","project":"globex-pilot","node":"T001","skill":"","text":""}'; wait_run ask-trace-1` | The run ends (`done`) |
| 4 | Run `echoctl traces --job yolo --limit 1`, then `echoctl traces show <its id>` | The newest trace is `ask-trace-1`'s: its "claude.run" has `attrs.echo.telemetry` "configured elsewhere", `attrs.args` with Yolo's prompt after "-p" (a Yolo run gets its prompt as an argument, so there is no "claude.input" event), and its stream's events from Echo ("claude.system.init" … "claude.result.success"); no span or event with `source` "claude-code" anywhere in the trace |
| 5 | `rm $ECHOCTL_CONFIG_DIR/claude/ollama/settings.json` | Removed |

## Expected Final State
One more Yolo run on Globex pilot's T001 (it changes nothing: its script only reads the task); the profile's settings as before.

## Teardown
- `rm -f $ECHOCTL_CONFIG_DIR/claude/ollama/settings.json`

## Notes
- The other places that configure telemetry — Echo's environment, the login shell's start-up files, a project's settings — and that sign-ins and version checks get none of Echo's: Rust tests (`telemetry::tests::echo_sets_no_telemetry_when_something_else_configures_claude_codes`, `ai::tests::through_the_login_shell_…`, `ai::tests::a_print_run_reports_its_telemetry_…`).
