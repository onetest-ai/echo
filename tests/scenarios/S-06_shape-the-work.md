---
id: S-06
title: Shape the work: the board, the tree, new items, edit, done, undo, link, merge and a rejected proposal
run: agent
data: small
cases:
  - tests/cases/work/TC-004_board-columns-and-cards.md
  - tests/cases/work/TC-001_saved-views-board-tree-new-rename-delete.md
  - tests/cases/work/TC-005_drag-card-sets-state.md
  - tests/cases/work/TC-012_details-owner-handed-over-due.md
  - tests/cases/work/TC-016_waits-for-holds-up-copy-link.md
  - tests/cases/work/TC-011_status-change-history-cli.md
  - tests/cases/work/TC-010_details-open-title-description.md
  - tests/cases/work/TC-014_who-does-it-toggle.md
  - tests/cases/work/TC-002_filters-kind-owner-open-only-group-by.md
  - tests/cases/work/TC-013_criteria-and-risk-levels.md
  - tests/cases/work/TC-007_tree-with-records.md
  - tests/cases/work/TC-006_column-menu.md
  - tests/cases/work/TC-017_link-to-an-item-in-another-thread.md
  - tests/cases/work/TC-015_history-sync-entry-quote-and-source.md
  - tests/cases/work/TC-003_sync-line.md
  - tests/cases/work/TC-008_to-review-accept-a-merge.md
  - tests/cases/work/TC-009_to-review-reject-a-merge.md
---

# S-06: Shape the work: the board, the tree, new items, edit, done, undo, link, merge and a rejected proposal

**Story:** Sam opens Acme rollout's Work tab, reads the board, filters and groups it, moves a card, changes a status and fields, links tasks, links one to another thread, checks the tree with records, and answers the proposals Echo found (accept one merge, reject another).

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–17 |
| Acme rollout's Work tab (Board) open | stage 1, step 1 | 2–8 (stage 8 leaves it for Tasks) |
| T001's Details open | stage 6, step 1 | 7, 8 |
| The decision "Pilot starts in Berlin" | stage 9's precondition (its Teardown's delete skipped) | 9, 11 (deleted in stage 11's Teardown) |
| The risk "SSO vendor delay" | stage 10's precondition (its Teardown's delete skipped) | 10, 11 (deleted in stage 11's Teardown) |
| Acme rollout's Board, then Tree | stage 9, step 1; stage 10, step 1 | 10; 11 |
| Globex pilot's Work tab (Board) open | stage 12, step 1 | 13, 14 |
| Acme rollout's ledger synced (the call's evidence written, its id printed) | stage 15's precondition | 15, 16, 17 |
| Acme rollout's Board open again | stage 15, step 1 | 16, 17 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `work/TC-004_board-columns-and-cards.md` | — | — |
| 2 | `work/TC-001_saved-views-board-tree-new-rename-delete.md` | — | On stage 1's view (Acme rollout's Board): skip the navigation of step 1; check its result on the snapshot |
| 3 | `work/TC-005_drag-card-sets-state.md` | — | On stage 2's view (Acme rollout's Board): skip the navigation of step 1; check its result on the snapshot |
| 4 | `work/TC-012_details-owner-handed-over-due.md` | — | On stage 3's view (Acme rollout's Board): run the case's two `project person` commands, reload the page, then skip the navigation of step 1 and click the card T002 |
| 5 | `work/TC-016_waits-for-holds-up-copy-link.md` | — | Its precondition (the CLI link), then on stage 4's view (Acme rollout's Board): skip the navigation of step 1, click the card T002 (the click reads the Details again) |
| 6 | `work/TC-011_status-change-history-cli.md` | — | On stage 5's view (Acme rollout's Board): skip the navigation of step 1, click the card T001 |
| 7 | `work/TC-010_details-open-title-description.md` | — | On stage 6's view (T001's Details open on the Board): skip step 1's navigation and click; check its result on the snapshot |
| 8 | `work/TC-014_who-does-it-toggle.md` | — | On stage 7's view (T001's Details open on the Board): skip step 1's navigation and click; check its result on the snapshot |
| 9 | `work/TC-002_filters-kind-owner-open-only-group-by.md` | — | Skip the first line of its Teardown (deleting the decision): the decision is kept for stage 11 |
| 10 | `work/TC-013_criteria-and-risk-levels.md` | — | Its precondition (the risk), then on stage 9's view (Acme rollout's Board, no filter): skip the navigation of step 1, click "Tree" and the row S001. Skip the last two lines of its Teardown (delete the risk, click "Board"): the risk and the Tree are kept for stage 11; untick the checkbox as it says |
| 11 | `work/TC-007_tree-with-records.md` | — | Its precondition creates only the question (the decision is stage 9's, the risk stage 10's). On stage 10's view (Acme rollout's Tree): skip step 1; check its header on the snapshot and wait until the row "Who signs off the pilot?" shows (the window reads the CLI's write within seconds) |
| 12 | `work/TC-006_column-menu.md` | — | — |
| 13 | `work/TC-017_link-to-an-item-in-another-thread.md` | — | On stage 12's view (Globex pilot's Board): skip the navigation of step 1, click the card T001 |
| 14 | `work/TC-015_history-sync-entry-quote-and-source.md` | — | Its preconditions (Globex pilot's sync and apply), then on stage 13's view (Globex pilot's Board): skip the navigation of step 1, wait for the card "Share the Globex pilot plan next week" and click it |
| 15 | `work/TC-003_sync-line.md` | — | — |
| 16 | `work/TC-008_to-review-accept-a-merge.md` | — | Skip the sync in its preconditions (stage 15 ran it; the evidence id is the one it printed under `pending`); run the apply. On stage 15's view (Acme rollout's Board): skip the navigation of step 1; wait for the "To review" banner |
| 17 | `work/TC-009_to-review-reject-a-merge.md` | — | Skip the sync in its preconditions (stage 15 ran it); run the apply with the same evidence id. On stage 16's view (Acme rollout's Board): skip the navigation of step 1; wait for the "To review" banner |

## Notes
- The stages are grouped by what they work on: Acme rollout's Board and its cards (1–8), its records on the Tree (9–11), Globex pilot's Board (12–14), then the proposals (15–17). Each view is reached once per group; a stage that goes on from the one before skips only its navigation (and a click when that Details panel is already open). If that view or state isn't there (an earlier stage failed), run the skipped steps and preconditions as written.
- The records are created once: stage 9 makes the decision, stage 10 the risk, stage 11 adds the question and deletes all three. Stage 9's Kind counts ("Decisions · 1", "Risks · 0", "Questions · 0") are read before the risk and the question exist; the question is a card on the Board, so it lives only in stage 11.
- Records and new tasks are created through `echoctl project ledger <id> edit` / `apply` (the window has no "new task" button); the window reads a CLI write within a few seconds (its polling), so a stage that goes on from the same view waits for the change instead of navigating.
- Records created by the cases (decision, risk, question) get the next free id, not always D001 / R001 / Q001: the cases name them by title; note each id as it is printed, for stage 11's Teardown.
- Stage 15 expects "Not synced yet · 1 call to read" and runs before stages 16 and 17, whose proposals add "to review" to the line. Acme rollout's sync runs once (stage 15); stages 16 and 17 apply with its evidence id.
- "Sync now", "Send to Claude" and every other button that starts Claude Code are never pressed.
- Stages 16 and 17 keep their changes (a merge, a task): they come last; the data are fresh for every scenario.
