---
id: S-04
title: The CLI on the same data: tasks by who has to move, the app's state
run: agent
data: small
cases:
  - tests/cases/cli/TC-001_project-tasks-piles.md
  - tests/cases/cli/TC-002_status-without-app.md
  - tests/cases/cli/TC-020_compose-puts-the-draft-where-drafts-go.md
auto: [tests/auto/specs/S-04.spec.mjs]
---

# S-04: The CLI on the same data: tasks by who has to move, the app's state

**Story:** Sam, in a terminal, asks Echo who owes what and what the app is doing.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build (no app run on it) | the lead | 1–2 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `cli/TC-001_project-tasks-piles.md` | — | — |
| 2 | `cli/TC-002_status-without-app.md` | — | — |
| 3 | `cli/TC-020_compose-puts-the-draft-where-drafts-go.md` | — | — |

## Notes
- CLI only: no browser needed, no prelude. Two read-only commands on one build: nothing to share or skip.
