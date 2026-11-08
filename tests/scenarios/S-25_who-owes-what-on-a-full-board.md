---
id: S-25
title: Who owes what on a full board: Tasks, piles, filters, badges, the board and its columns
run: agent
data: standard
cases:
  - tests/cases/tasks/TC-015_i-owe-lists-my-open-tasks-blocked-ones-marked.md
  - tests/cases/tasks/TC-018_owed-to-me-and-others-split-by-who-handed-it-over.md
  - tests/cases/tasks/TC-016_waiting-for-others-work-my-tasks-wait-on-and-a-card-per-person.md
  - tests/cases/tasks/TC-017_nobody-named-takes-every-task-without-an-owner.md
  - tests/cases/tasks/TC-019_closed-tasks-are-in-no-pile.md
  - tests/cases/tasks/TC-020_due-chip-late-today-this-week-later-and-no-date.md
  - tests/cases/tasks/TC-021_quiet-mark-only-on-undated-tasks-five-days-quiet.md
  - tests/cases/tasks/TC-022_order-due-quiet-longest-and-thread-on-a-full-pile.md
  - tests/cases/tasks/TC-023_who-does-it-filter-on-the-full-board.md
  - tests/cases/tasks/TC-024_thread-filter-counts-and-the-cards-of-each-thread.md
  - tests/cases/tasks/TC-025_asks-by-person-on-the-full-board.md
  - tests/cases/work/TC-019_board-custom-columns-limit-hidden-and-fallback.md
  - tests/cases/work/TC-024_board-filters-owner-and-kind-hold-in-the-done-column.md
  - tests/cases/work/TC-027_sync-line-on-four-threads.md
  - tests/cases/work/TC-018_board-default-columns-where-each-card-lands.md
  - tests/cases/work/TC-025_board-card-avatar-and-chips-late-waiting-holds-up.md
  - tests/cases/work/TC-026_details-show-links-across-threads.md
  - tests/cases/work/TC-020_board-column-counts-in-four-threads.md
  - tests/cases/work/TC-021_tree-status-words-questions-and-risks.md
  - tests/cases/work/TC-023_merged-and-deleted-items-show-nowhere.md
  - tests/cases/work/TC-022_tree-status-words-tasks-and-stories.md
  - tests/cases/work/TC-029_owner-filter-lists-the-user-once-and-keeps-all-their-items.md
---

# S-25: Who owes what on a full board: Tasks, piles, filters, badges, the board and its columns

**Story:** Sam's week has run for a while: 57 open tasks across seven threads, closed ones left behind, work waiting on other people and on other threads. He opens Tasks and checks where each of his own tasks has landed (I owe, with the blocked ones marked), what he handed over (Owed to me) and what is only other people's work (Others), what waits on others (Waiting for others, a card per person), who has nothing named. He reads the due chips (late in red, the weekday this week), the quiet marks, the order, the Who does it and Thread filters and the Asks by person. Then he goes into the threads' Work tabs: a board with custom columns and its filters, what the sync line says, where each card sits on the default board and what the cards show, which links reach other threads, the column counts of four threads, and last how the Tree words questions, risks, tasks and stories, and that merged and deleted items are shown nowhere; last, that the Owner filter lists him once.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run; every stage only reads, or changes a view setting.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Standard data, built right before the run | `serve.sh up <dir> <port> --set standard` | all |
| Tasks with every pile unfolded ("+ 8" in I owe, "Others · 24", "+ 5" in Nobody named) | stage 1 step 2, stage 2 step 3, stage 4 step 2 | 3, 5–11 (no stage navigates to `#tasks` again or folds a pile) |
| Acme rollout's board | stage 12, step 1 | 13, 14 |
| Globex pilot's work tab | stage 14, step 5 | 15, 16, 17 |
| Acme renewal's work tab (Details on T001) | stage 17, step 4 | 18 |
| Umbrella proposal's board | stage 18, step 5 | 19 |
| "Open only" off in the Tree of the five threads (a saved view setting) | stage 19 (steps 1 and 3) | 20, 21 (each tree is already as they need it; nobody turns it back on) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `tasks/TC-015_i-owe-lists-my-open-tasks-blocked-ones-marked.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `tasks/TC-018_owed-to-me-and-others-split-by-who-handed-it-over.md` | — | On stage 1's view: skip step 1's navigation; check its expected result |
| 3 | `tasks/TC-016_waiting-for-others-work-my-tasks-wait-on-and-a-card-per-person.md` | — | On stage 2's view: skip step 1's navigation; in step 3 skip the click (Others is open since stage 2) and look |
| 4 | `tasks/TC-017_nobody-named-takes-every-task-without-an-owner.md` | — | Skip step 1's navigation |
| 5 | `tasks/TC-019_closed-tasks-are-in-no-pile.md` | — | On stage 4's view: in step 1 skip the navigation and the three clicks (every pile is unfolded); count the cards |
| 6 | `tasks/TC-020_due-chip-late-today-this-week-later-and-no-date.md` | — | Skip step 1 (all piles are unfolded) |
| 7 | `tasks/TC-021_quiet-mark-only-on-undated-tasks-five-days-quiet.md` | — | Skip step 1 |
| 8 | `tasks/TC-022_order-due-quiet-longest-and-thread-on-a-full-pile.md` | — | Skip step 1 (I owe is unfolded, Order is "Due") |
| 9 | `tasks/TC-023_who-does-it-filter-on-the-full-board.md` | — | Skip step 1 |
| 10 | `tasks/TC-024_thread-filter-counts-and-the-cards-of-each-thread.md` | — | In step 1 skip the navigation; read the options |
| 11 | `tasks/TC-025_asks-by-person-on-the-full-board.md` | — | In step 1 skip the navigation; open the Inbox panel and its tab "Asks" |
| 12 | `work/TC-019_board-custom-columns-limit-hidden-and-fallback.md` | — | Navigate to `{{base_url}}/#projects` so the thread list is loaded |
| 13 | `work/TC-024_board-filters-owner-and-kind-hold-in-the-done-column.md` | — | On stage 12's view: in step 1 skip the navigation and the Board click (Acme rollout's board is shown) |
| 14 | `work/TC-027_sync-line-on-four-threads.md` | — | On stage 13's view: skip step 1's navigation (Acme rollout's work tab is shown) |
| 15 | `work/TC-018_board-default-columns-where-each-card-lands.md` | — | On stage 14's view: in step 1 skip the navigation (Globex pilot's work tab is shown) |
| 16 | `work/TC-025_board-card-avatar-and-chips-late-waiting-holds-up.md` | — | On stage 15's view: skip step 1 |
| 17 | `work/TC-026_details-show-links-across-threads.md` | — | On stage 16's view: in step 1 skip the navigation and the Board click; click the card T005 |
| 18 | `work/TC-020_board-column-counts-in-four-threads.md` | — | On stage 17's view: in step 1 skip the navigation (Acme renewal's work tab is shown) |
| 19 | `work/TC-021_tree-status-words-questions-and-risks.md` | — | On stage 18's view: in step 1 skip the navigation (Umbrella proposal's work tab is shown). **Skip its teardown**: "Open only" stays off for stages 20 and 21 |
| 20 | `work/TC-023_merged-and-deleted-items-show-nowhere.md` | — | On stage 19's view: skip step 1 (Acme rollout's Tree is shown with "Open only" off). **Skip its teardown** |
| 21 | `work/TC-022_tree-status-words-tasks-and-stories.md` | — | On stage 20's view: in step 1 skip the navigation (Acme rollout's work tab is shown); click "Tree" ("Open only" is already off). **Skip its teardown** |
| 22 | `work/TC-029_owner-filter-lists-the-user-once-and-keeps-all-their-items.md` | — | — (the last stage; Acme rollout's Tree has "Open only" off since stage 19: step 4 leaves it off) |

## Notes
- A stage that continues an earlier stage's view ("On stage N's view") takes one snapshot first; if the state is not there (the earlier stage failed before it), it does the skipped steps after all.
- Stages 1 to 11 stay on Tasks: the piles are unfolded once (stages 1, 2, 4) and stay so; the filters and the order are changed and put back inside each stage.
- Stage 13 (`work/TC-024`) sets and clears its filters, so stages 14 and 17 see Acme rollout's board unfiltered.
- The boards are read before any Tree: stages 19 to 21 turn "Open only" off in the Trees of the five threads (a saved view setting) and leave it off, since no stage after them reads a board with "Open only" on (stage 22 reads Acme rollout's Board, where "Open only" is still on, and its Tree with it off). Each thread is visited once per stage: `work/TC-020`, `TC-021` and `TC-022` go thread by thread, and `work/TC-027` reads Acme rollout's two lines together.
- Not covered: Today's "My work · this week" (being reworked), the Claude dialog and anything that starts Claude Code (Sync now, Send to Claude).
