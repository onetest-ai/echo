---
id: TC-020
title: Live notes on Claude are kept by one Claude Code session for the whole recording, through its two tools, and it ends with the call
priority: high
type: functional
requirements: [F-LIVE-09]
module: claude
run: agent
platform: macos
tags: [live, claude-code, performance, file-fed]
---

# TC-020: One Claude session keeps a call's live notes through its tools

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Claude Code installed and **signed in** on this Mac (the run costs well under a cent). Test data from `tests/fixtures/make_data.py <dir> --ai fake`, calls fed from files (app_profile.md → "Recording with file-fed calls"), `target/debug/echoctl` built from this checkout.
- The prelude: `. <dir>/env; export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v "^<dir>" | paste -sd: -); unset CLAUDE_CONFIG_DIR; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py` (the real Claude Code, its own sign-in, not the data's fake)
- The speech model linked into the test config (`recording/TC-015`'s preconditions); a second app instance on the test data

## Test Data

| Field | Value |
|-------|-------|
| AI profile | `system` (Claude Code as configured), the default |
| Conversation | minutes-call (213 s) |
| Arm title | AGENT-020 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl ai default system`, `target/debug/echoctl ai live on`, `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`; start the app (`nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 &`), `$W 30 app`, `target/debug/echoctl arm --title AGENT-020`, `$C start minutes-call` | The call records |
| 2 | `S=$(basename $(ls -d $ECHOCTL_HOME/calls/*-agent-020))`; every 2 s for 4 minutes: count the distinct pids of `pgrep -f "minutes-run/live-${S:0:17}"` (this call's Claude Code: its MCP config is named after the call's stamp) and `pgrep -f "live-mcp --call .*$S"`, and note each change of `live-notes.json`'s `updated_at` (only this call's: the user's own Echo may run sessions of its own) | **1** Claude Code process and **1** `live-mcp` server for the whole call; the notes are updated 2 or more times (about every 90 s) |
| 3 | After the call ended (+4 min): read `live-notes.json` and `live-usage.json` of the call; `pgrep -f "minutes-run/live-${S:0:17}"`, `pgrep -f "live-mcp --call .*$S"` | The notes cover most of the call (`covered_until` over 180 s of 213), with decisions; usage shows `cache_read_tokens` above zero; no Claude Code and no `live-mcp` left running |

## Expected Final State
The call's live notes came from one Claude Code session's tool calls; nothing of it runs after the call.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill` its pid

## Notes
- Measured 2026-11-09 (M4 Max): 1 process, updates at about +48/+140/+230 s and a last one after the call, covering 211 of 213 s; cache read 11,868–29,813 tokens; about $0.002; the notes in `transcript.md` after the call; nothing left running. Before (#239): a Claude Code start for every summary, each about 4.7 s of CPU and 400 MB.
- A model server (Ollama) profile isn't affected: its summaries are HTTP requests, no Claude Code.
