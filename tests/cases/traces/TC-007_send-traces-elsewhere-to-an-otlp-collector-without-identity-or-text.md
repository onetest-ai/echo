---
id: TC-007
title: Send traces elsewhere — set up an OTLP collector in Settings → Activity, a test send reaches it without the account's identity or the run's text, the headers kept and never shown, Turn off stops it
priority: critical
type: functional
requirements: [F-TRACE-09]
module: traces
run: agent
platform: macos
data: standard
ai: virtual
tags: [traces, export, otlp, langfuse, privacy, window]
---

# TC-007: Send traces elsewhere to an OTLP collector

**Module:** Traces | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; `{{base_url}}/` loaded first; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env` (abort if `ECHOCTL_HOME` isn't inside `<dir>`)
- Acme rollout's sync ran on this data (`work/TC-028`, its stage in S-33): its trace is there
- The stand-in collector, started for this case: `nohup python3 tests/fixtures/fake_otlp.py <port + 1> <dir>/otlp.jsonl > /dev/null 2>&1 &` (note its pid); it writes each request it gets to `<dir>/otlp.jsonl`
- Besides `echoctl`, this case may run `python3 -c` and `grep` over `<dir>/otlp.jsonl`, and `kill` the collector's pid

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl traces export` | `on` false, `kind` "", `secret_key_set` false |
| 2 | Navigate to `{{base_url}}/#settings/activity`; click "Send traces elsewhere…" under the heading | "Activity › Send traces elsewhere"; the heading "Send traces elsewhere" saying it is off until set up and never sends the account's identity; "Send to" Langfuse (pressed) or OTLP collector; Langfuse host `https://cloud.langfuse.com`, Public key, Secret key ("Kept on this computer, readable only by you."); the switch "What went in and came out" off; "Turn on" |
| 3 | Click "OTLP collector"; type `http://collector.acme.example:4318` as the Collector; click "Turn on" | Refused, saying it isn't an address traces may go to (https://, or http:// on this computer); still "Not sending." |
| 4 | Type `http://127.0.0.1:<port + 1>` as the Collector and `X-Team: acme` in Headers; click "Turn on" | "Sending to your collector"; "Save", "Turn off", "Send a test"; Headers say "Kept: X-Team — type them again to change them." (the value is never shown) |
| 5 | Click "Send a test" | "Sent ✓" within a few seconds |
| 6 | Read `<dir>/otlp.jsonl`'s last line with `python3 -c` | `path` "/v1/traces"; `headers` has `X-Team: acme` and `Content-Type: application/json`; `body.resourceSpans` with `service.name` "echo" and "claude-code"; a span "job.thread_sync" with the attribute `langfuse.trace.name`, "claude.run" spans with `langfuse.observation.type` "generation" and `gen_ai.usage.input_tokens`; `grep -c` finds none of "@", "Acme security review", the run's folder path or "user.email" in that line |
| 7 | Run `echoctl traces export`; then `ls -l $ECHOCTL_CONFIG_DIR/telemetry-export.json` | `on` true, `kind` "otlp", `headers` `["X-Team"]` (names only); the file `-rw-------` (only the user) |
| 8 | Back in the form, click "Turn off"; then "Activity" in the path | "Not sending."; under the heading "Send traces elsewhere…" again |
| 9 | `kill` the collector | Stopped |

## Expected Final State
Sending is off; the collector stopped.

## Teardown
- `kill` the collector if it still runs; `rm -f $ECHOCTL_CONFIG_DIR/telemetry-export.json`

## Notes
- Langfuse's path (`/api/public/otel/v1/traces`), its Basic auth and ingestion header; what went in and came out sent only with the switch on; only what ended since sending was turned on, a minute after it ended, once each; a failure kept and tried again: Rust tests (`telemetry::export::tests`).
