---
id: TC-014
title: With no open task Tasks says so
priority: low
type: functional
requirements: [F-TASK-09]
module: tasks
run: agent
platform: any
tags: [tasks, empty]
auto: [tests/auto/specs/S-08.spec.mjs]
---

# TC-014: With no open task Tasks says so

**Module:** Tasks | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Run `echoctl project tasks`; for every task in every pile mark it done: `echoctl project ledger <project> edit '{"op":"edit","node":"<id>","set":{"state":"done"}}'` (note each project and id). `echoctl project tasks` then lists no task. Every `echoctl` command runs after `. <dir>/env` (the run's env file)
- In S-08 Acme rollout's T001 is already done (stage 12, tasks/TC-012, kept it closed): note it with the others, so the Teardown reopens it too

## Test Data

| Field | Value |
|-------|-------|
| Tasks | every open task of Acme rollout and Globex pilot |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" (or reload `{{base_url}}/#tasks`) | The heading "Tasks" and the filters are shown |
| 2 | Read the body | "No open tasks" and the text "Once a thread's calls are synced, what was promised, owed and waited on shows here — from every thread at once."; no pile heading "I owe" |

## Expected Final State
Tasks shows No open tasks and no pile.

## Teardown
- For each project and id noted: `echoctl project ledger <project> edit '{"op":"edit","node":"<id>","set":{"state":"open"}}'`

## Notes
- Changes every open task's state (reopened in Teardown).
