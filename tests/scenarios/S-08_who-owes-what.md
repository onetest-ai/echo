---
id: S-08
title: Who owes what: the piles, the filters, a card, closing a task, Claude's dialog, Asks
run: agent
data: small
cases:
  - tests/cases/tasks/TC-001_piles-and-counts.md
  - tests/cases/tasks/TC-002_asks-empty.md
  - tests/cases/tasks/TC-003_filter-by-thread.md
  - tests/cases/tasks/TC-004_card-opens-details.md
  - tests/cases/tasks/TC-005_card-facts.md
  - tests/cases/tasks/TC-006_order.md
  - tests/cases/tasks/TC-007_owed-to-me-pile.md
  - tests/cases/tasks/TC-008_asks-by-person.md
  - tests/cases/tasks/TC-009_who-does-it-filter.md
  - tests/cases/tasks/TC-010_send-to-claude-dialog.md
  - tests/cases/tasks/TC-011_pile-folds-after-six.md
  - tests/cases/tasks/TC-012_close-from-today.md
  - tests/cases/tasks/TC-013_no-active-threads.md
  - tests/cases/tasks/TC-014_no-open-tasks.md
---

# S-08: Who owes what: the piles, the filters, a card, closing a task, Claude's dialog, Asks

**Story:** Sam opens Tasks to see what he owes and what others owe him, narrows it by thread and by order, opens a card, hands a task of Omar's over and sees it in Asks, looks at Claude's dialog for a task and cancels it, closes a task from Today with a comment, and sees what Tasks says when nothing is left.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `tasks/TC-001_piles-and-counts.md` | — | Reload `{{base_url}}/` (the cookie) |
| 2 | `tasks/TC-002_asks-empty.md` | — | — |
| 3 | `tasks/TC-003_filter-by-thread.md` | — | — |
| 4 | `tasks/TC-004_card-opens-details.md` | — | — |
| 5 | `tasks/TC-005_card-facts.md` | — | — |
| 6 | `tasks/TC-006_order.md` | — | — |
| 7 | `tasks/TC-007_owed-to-me-pile.md` | — | — |
| 8 | `tasks/TC-008_asks-by-person.md` | — | — |
| 9 | `tasks/TC-009_who-does-it-filter.md` | — | — |
| 10 | `tasks/TC-010_send-to-claude-dialog.md` | — | — |
| 11 | `tasks/TC-011_pile-folds-after-six.md` | — | — |
| 12 | `tasks/TC-012_close-from-today.md` | — | — |
| 13 | `tasks/TC-013_no-active-threads.md` | — | — |
| 14 | `tasks/TC-014_no-open-tasks.md` | — | — |

## Notes
- Stage 1 and 2 read the data as built (Omar's task in Others, nothing owed to Sam): they come first because stages 7 and 8 hand it over. Every stage that edits the ledger undoes it in its Teardown, so the next stage starts from the same tasks.
- Stage 5 and 6 make a task of Globex pilot overdue and restore it; stage 10 creates and cancels a task; stage 11 creates and cancels tasks; stage 12 closes and reopens one; stages 13 and 14 close and reopen threads and tasks.
- No stage presses Work on it, Yolo or Sync now: none starts Claude Code.
- Stage 12 holds a suspected gap (the close comment is not shown in the Details History); its case reads the comment through the CLI.
