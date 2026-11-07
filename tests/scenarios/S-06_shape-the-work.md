---
id: S-06
title: Shape the work: the board, the tree, new items, edit, done, undo, link, merge and a rejected proposal
run: agent
data: small
cases:
  - tests/cases/work/TC-004_board-columns-and-cards.md
  - tests/cases/work/TC-002_filters-kind-owner-open-only-group-by.md
  - tests/cases/work/TC-001_saved-views-board-tree-new-rename-delete.md
  - tests/cases/work/TC-006_column-menu.md
  - tests/cases/work/TC-005_drag-card-sets-state.md
  - tests/cases/work/TC-011_status-change-history-cli.md
  - tests/cases/work/TC-012_details-owner-handed-over-due.md
  - tests/cases/work/TC-014_who-does-it-toggle.md
  - tests/cases/work/TC-010_details-open-title-description.md
  - tests/cases/work/TC-016_waits-for-holds-up-copy-link.md
  - tests/cases/work/TC-017_link-to-an-item-in-another-thread.md
  - tests/cases/work/TC-013_criteria-and-risk-levels.md
  - tests/cases/work/TC-007_tree-with-records.md
  - tests/cases/work/TC-003_sync-line.md
  - tests/cases/work/TC-015_history-sync-entry-quote-and-source.md
  - tests/cases/work/TC-008_to-review-accept-a-merge.md
  - tests/cases/work/TC-009_to-review-reject-a-merge.md
---

# S-06: Shape the work: the board, the tree, new items, edit, done, undo, link, merge and a rejected proposal

**Story:** Sam opens Acme rollout's Work tab, reads the board, filters and groups it, moves a card, changes a status and fields, links tasks, links one to another thread, checks the tree with records, and answers the proposals Echo found (accept one merge, reject another).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `work/TC-004_board-columns-and-cards.md` | — | — |
| 2 | `work/TC-002_filters-kind-owner-open-only-group-by.md` | — | — |
| 3 | `work/TC-001_saved-views-board-tree-new-rename-delete.md` | — | — |
| 4 | `work/TC-006_column-menu.md` | — | — |
| 5 | `work/TC-005_drag-card-sets-state.md` | — | — |
| 6 | `work/TC-011_status-change-history-cli.md` | — | — |
| 7 | `work/TC-012_details-owner-handed-over-due.md` | — | — |
| 8 | `work/TC-014_who-does-it-toggle.md` | — | — |
| 9 | `work/TC-010_details-open-title-description.md` | — | — |
| 10 | `work/TC-016_waits-for-holds-up-copy-link.md` | — | — |
| 11 | `work/TC-017_link-to-an-item-in-another-thread.md` | — | — |
| 12 | `work/TC-013_criteria-and-risk-levels.md` | — | — |
| 13 | `work/TC-007_tree-with-records.md` | — | Click "Board" if the Tree is active |
| 14 | `work/TC-003_sync-line.md` | — | — |
| 15 | `work/TC-015_history-sync-entry-quote-and-source.md` | — | — |
| 16 | `work/TC-008_to-review-accept-a-merge.md` | — | — |
| 17 | `work/TC-009_to-review-reject-a-merge.md` | — | — |

## Notes
- Records and new tasks are created through `echoctl project ledger <id> edit` / `apply` (the window has no "new task" button).
- Stage 14 expects "Not synced yet · 1 call to read" and runs before stages 16 and 17, whose proposals add "to review" to the line.
- Records created by the cases (decision, risk, question) get the next free id, not always D001 / R001 / Q001: the cases name them by title.
- "Sync now", "Send to Claude" and every other button that starts Claude Code are never pressed.
- Stages 15, 16 and 17 keep their changes (a task, a merge, a task): they come last; the data are fresh for every scenario.
