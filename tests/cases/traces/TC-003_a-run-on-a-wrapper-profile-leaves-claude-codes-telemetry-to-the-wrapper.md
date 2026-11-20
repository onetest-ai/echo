---
id: TC-003
title: A run on a wrapper profile leaves Claude Code's telemetry to the wrapper; its trace has Echo's part
priority: medium
type: functional
requirements: [F-TRACE-04]
module: traces
run: person
platform: macos
tags: [traces, telemetry, wrapper, claude-code]
---

# TC-003: A run on a wrapper profile leaves Claude Code's telemetry to the wrapper

**Module:** Traces | **Priority:** Medium | **Type:** Functional

## Preconditions
- A Mac with Echo built from this branch and a wrapper profile (a company's Claude Code command, added in Settings → AI & minutes → Add a wrapper…) as the default profile, signed in

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Give a small task to Claude with Yolo (Today → My work → a task's Claude button → Yolo) and wait until it ends | The run ends as before this change; the wrapper's own reporting (its usage page, if it has one) shows the run as before |
| 2 | Run `echoctl traces --job yolo --limit 1`, then `echoctl traces show <its id>` | One "job.yolo" span with a "claude.run" whose `attrs.echo.telemetry` is "the wrapper's own", its events from Echo ("claude.system.init" … "claude.result.success") and its tokens and cost; no span with `source` "claude-code" |

## Expected Final State
One Yolo run done.

## Teardown
- None
