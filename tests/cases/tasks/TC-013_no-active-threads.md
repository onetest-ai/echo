---
id: TC-013
title: With no active thread Tasks offers Open Threads
priority: low
type: functional
requirements: [F-TASK-09]
module: tasks
run: agent
platform: any
tags: [tasks, empty]
---

# TC-013: With no active thread Tasks offers Open Threads

**Module:** Tasks | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Both threads are closed: `echoctl project set acme-rollout --stage done` and `echoctl project set globex-pilot --stage done`. Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Threads | Acme rollout, Globex pilot |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" (or reload `{{base_url}}/#tasks`) | The text "across 0 active projects" |
| 2 | Read the body | "No active threads", the text "Tasks come from your threads' calls: each thread's Work tab reads them into stories and tasks." and a button "Open Threads" |
| 3 | Click "Open Threads" | The Threads view opens (URL starts with `#projects` or `#project/`) |

## Expected Final State
Tasks showed its no-thread state and the button led to Threads.

## Teardown
- `echoctl project set acme-rollout --stage active` and `echoctl project set globex-pilot --stage active`

## Notes
- Changes both threads' stage (undone in Teardown).
