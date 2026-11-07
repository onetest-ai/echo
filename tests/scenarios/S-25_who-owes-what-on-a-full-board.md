---
id: S-25
title: Who owes what on a full board: Tasks, piles, filters, badges, the board and its columns
run: agent
data: standard
cases:
  - tests/cases/tasks/TC-015_i-owe-lists-my-open-tasks-blocked-ones-marked.md
  - tests/cases/tasks/TC-016_waiting-for-others-work-my-tasks-wait-on-and-a-card-per-person.md
  - tests/cases/tasks/TC-017_nobody-named-takes-every-task-without-an-owner.md
  - tests/cases/tasks/TC-018_owed-to-me-and-others-split-by-who-handed-it-over.md
  - tests/cases/tasks/TC-019_closed-tasks-are-in-no-pile.md
  - tests/cases/tasks/TC-020_due-chip-late-today-this-week-later-and-no-date.md
  - tests/cases/tasks/TC-021_quiet-mark-only-on-undated-tasks-five-days-quiet.md
  - tests/cases/tasks/TC-022_order-due-quiet-longest-and-thread-on-a-full-pile.md
  - tests/cases/tasks/TC-023_who-does-it-filter-on-the-full-board.md
  - tests/cases/tasks/TC-024_thread-filter-counts-and-the-cards-of-each-thread.md
  - tests/cases/tasks/TC-025_asks-by-person-on-the-full-board.md
  - tests/cases/work/TC-018_board-default-columns-where-each-card-lands.md
  - tests/cases/work/TC-019_board-custom-columns-limit-hidden-and-fallback.md
  - tests/cases/work/TC-020_board-column-counts-in-four-threads.md
  - tests/cases/work/TC-025_board-card-avatar-and-chips-late-waiting-holds-up.md
  - tests/cases/work/TC-024_board-filters-owner-and-kind-hold-in-the-done-column.md
  - tests/cases/work/TC-021_tree-status-words-questions-and-risks.md
  - tests/cases/work/TC-022_tree-status-words-tasks-and-stories.md
  - tests/cases/work/TC-023_merged-and-deleted-items-show-nowhere.md
  - tests/cases/work/TC-026_details-show-links-across-threads.md
  - tests/cases/work/TC-027_sync-line-on-four-threads.md
---

# S-25: Who owes what on a full board: Tasks, piles, filters, badges, the board and its columns

**Story:** Sam's week has run for a while: 57 open tasks across seven threads, closed ones left behind, work waiting on other people and on other threads. He opens Tasks and checks where each of his own tasks has landed (I owe, with the blocked ones marked), what waits on others (Waiting for others, a card per person), who has nothing named, what he handed over (Owed to me) and what is only other people's work (Others). He reads the due chips (late in red, the weekday this week), the quiet marks, the order, the Who does it and Thread filters and the Asks by person. Then he goes into the threads' Work tabs: where each card sits on the default board and on a board with custom columns, what the cards show, how the Tree words questions, risks, tasks and stories, what the sync line says, which links reach other threads, and that merged and deleted items are shown nowhere.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run; every stage only reads, or restores what it changes.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `tasks/TC-015_i-owe-lists-my-open-tasks-blocked-ones-marked.md` | — | Reload `{{base_url}}/` (the cookie) |
| 2 | `tasks/TC-016_waiting-for-others-work-my-tasks-wait-on-and-a-card-per-person.md` | — | — |
| 3 | `tasks/TC-017_nobody-named-takes-every-task-without-an-owner.md` | — | — |
| 4 | `tasks/TC-018_owed-to-me-and-others-split-by-who-handed-it-over.md` | — | — |
| 5 | `tasks/TC-019_closed-tasks-are-in-no-pile.md` | — | — |
| 6 | `tasks/TC-020_due-chip-late-today-this-week-later-and-no-date.md` | — | — |
| 7 | `tasks/TC-021_quiet-mark-only-on-undated-tasks-five-days-quiet.md` | — | — |
| 8 | `tasks/TC-022_order-due-quiet-longest-and-thread-on-a-full-pile.md` | — | — |
| 9 | `tasks/TC-023_who-does-it-filter-on-the-full-board.md` | — | — |
| 10 | `tasks/TC-024_thread-filter-counts-and-the-cards-of-each-thread.md` | — | — |
| 11 | `tasks/TC-025_asks-by-person-on-the-full-board.md` | — | — |
| 12 | `work/TC-018_board-default-columns-where-each-card-lands.md` | — | Navigate to `{{base_url}}/#projects` so the thread list is loaded |
| 13 | `work/TC-019_board-custom-columns-limit-hidden-and-fallback.md` | — | — |
| 14 | `work/TC-020_board-column-counts-in-four-threads.md` | — | — |
| 15 | `work/TC-025_board-card-avatar-and-chips-late-waiting-holds-up.md` | — | — |
| 16 | `work/TC-024_board-filters-owner-and-kind-hold-in-the-done-column.md` | — | — |
| 17 | `work/TC-021_tree-status-words-questions-and-risks.md` | — | — |
| 18 | `work/TC-022_tree-status-words-tasks-and-stories.md` | — | — |
| 19 | `work/TC-023_merged-and-deleted-items-show-nowhere.md` | — | — |
| 20 | `work/TC-026_details-show-links-across-threads.md` | — | — |
| 21 | `work/TC-027_sync-line-on-four-threads.md` | — | — |

## Notes
- Stage 16 (`work/TC-024`) is expected to FAIL until it is decided whether the Done column of a board follows the owner and kind filters; no stage depends on it.
- Stages 17 to 19 turn "Open only" off in a Tree view and back on; each restores it in its teardown.
- Not covered: Today's "My work · this week" (being reworked), the Claude dialog and anything that starts Claude Code (Sync now, Send to Claude).
