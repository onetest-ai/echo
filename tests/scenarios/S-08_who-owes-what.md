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

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–14 |
| Tasks open at `#tasks` | the prelude | 1–4 |
| Globex pilot's T001 overdue | stage 5's precondition (its Teardown skipped) | 5, 6 (put back in stage 6's Teardown) |
| Omar's task handed over by Sam and late | stage 7's precondition (its Teardown skipped) | 7, 8 (put back in stage 8's Teardown) |
| The task "Write the follow-up mail to Acme" (T003) | stage 10's precondition (its cancel skipped) | 10, 11 (cancelled in stage 11's Teardown) |
| Acme rollout's T001 closed from Today with a comment | stage 12, step 4 (its undo skipped) | 12, 14 (reopened in stage 14's Teardown) |

## Prelude
1. Load `{{base_url}}/` (the cookie), then navigate to `{{base_url}}/#tasks` and wait for the text "I owe".

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `tasks/TC-001_piles-and-counts.md` | — | On the prelude's view (`#tasks`): skip the navigation of step 1; check its result on the snapshot |
| 2 | `tasks/TC-002_asks-empty.md` | — | On stage 1's view (`#tasks`, Others folded): skip step 1 |
| 3 | `tasks/TC-003_filter-by-thread.md` | — | On stage 2's view (`#tasks`, the panel closed by its Teardown): skip the navigation of step 1; check its result on the snapshot |
| 4 | `tasks/TC-004_card-opens-details.md` | — | On stage 3's view (`#tasks`, All threads): skip the navigation of step 1; check its result on the snapshot |
| 5 | `tasks/TC-005_card-facts.md` | — | Skip its Teardown (keep Globex pilot's T001 overdue for stage 6) |
| 6 | `tasks/TC-006_order.md` | — | Skip its precondition (T001 is overdue since stage 5). On stage 5's view (`#tasks`, freshly reloaded): skip the navigation of step 1; check its result on the snapshot. Its Teardown puts the due date back |
| 7 | `tasks/TC-007_owed-to-me-pile.md` | — | Skip its Teardown (keep Omar's task handed over and late for stage 8) |
| 8 | `tasks/TC-008_asks-by-person.md` | — | Skip its precondition (the hand-over is stage 7's). On stage 7's view (`#tasks`, Owed to me 1): skip the navigation of step 1; check its result on the snapshot. Its Teardown puts the task back |
| 9 | `tasks/TC-009_who-does-it-filter.md` | — | — |
| 10 | `tasks/TC-010_send-to-claude-dialog.md` | — | Skip the first line of its Teardown (keep T003 open: stage 11 counts it and cancels it); close the panel |
| 11 | `tasks/TC-011_pile-folds-after-six.md` | — | Its precondition counts T003 (stage 10) in `piles.mine`, so one task fewer is created. Its Teardown also cancels T003 |
| 12 | `tasks/TC-012_close-from-today.md` | — | Skip its Teardown (keep T001 of Acme rollout done for stage 14; note the `mid` anyway) |
| 13 | `tasks/TC-013_no-active-threads.md` | — | — |
| 14 | `tasks/TC-014_no-open-tasks.md` | — | Its precondition finds T001 of Acme rollout already done (stage 12) and marks the others done; note T001 with them, so its Teardown reopens it too |

## Notes
- Stages 1 to 4 read the data as built on one visit to Tasks (the prelude's). They come first because stages 7 and 8 hand Omar's task over.
- A change two stages need is made once and undone once: Globex pilot's overdue T001 (stages 5–6), Omar's hand-over (7–8), T003 (10–11), the closed Acme T001 (12–14). Stages 9 and 13 undo their own change. If a kept change isn't there (its stage failed before making it), make it as the case's own Preconditions say.
- Stage 5 reloads the window fully (its step 1): a CLI change to a due date isn't seen by a hash-only navigation. Stage 6 reads that reloaded view.
- Stage 13 closes and reopens both threads while Acme rollout's T001 stays done from stage 12; Tasks then shows "No active threads" either way.
- No stage presses Work on it, Yolo or Sync now: none starts Claude Code.
- Stage 12 holds a suspected gap (the close comment is not shown in the Details History); its case reads the comment through the CLI.
