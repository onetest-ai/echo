---
id: S-04
title: The CLI on the same data: tasks by who has to move, the app's state
run: agent
data: standard
cases:
  - tests/cases/cli/TC-001_project-tasks-piles.md
  - tests/cases/cli/TC-002_status-without-app.md
---

# S-04: The CLI on the same data: tasks by who has to move, the app's state

**Story:** Sam, in a terminal, asks Echo who owes what and what the app is doing.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `cli/TC-001_project-tasks-piles.md` | — | — |
| 2 | `cli/TC-002_status-without-app.md` | — | — |

## Notes
- CLI only: no browser needed.
