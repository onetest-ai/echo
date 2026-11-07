---
id: S-28
title: "Echo from the terminal: help, threads, links, sends, series and settings, clients, thread changes, the work and one call"
run: agent
data: standard
cases:
  - tests/cases/cli/TC-007_help-lists-the-commands-and-read-commands-print-json.md
  - tests/cases/cli/TC-008_project-list-show-of-people-found-match-the-window.md
  - tests/cases/cli/TC-013_read-gives-the-text-behind-each-kind-of-echo-link.md
  - tests/cases/shell/TC-016_echo-links-open-their-place-in-the-window.md
  - tests/cases/cli/TC-016_m365-sends-nothing-without-yes.md
  - tests/cases/cli/TC-014_series-and-config-show-and-change-what-the-window-shows.md
  - tests/cases/cli/TC-015_client-and-org-commands-change-what-the-window-shows.md
  - tests/cases/cli/TC-009_project-create-set-rule-include-exclude-reset-delete.md
  - tests/cases/cli/TC-010_ledger-tree-and-edit-change-the-work-as-the-user.md
  - tests/cases/cli/TC-011_call-commands-context-actions-type-title-html.md
  - tests/cases/cli/TC-012_delete-restore-and-note-from-the-cli.md
---

# S-28: Echo from the terminal: help, threads, links, sends, series and settings, clients, thread changes, the work and one call

**Story:** Sam works from a terminal (or a Claude Code session does it for him) with the window open beside it. He looks up what `echoctl` can do, lists his threads and checks them against the window, reads what is behind a few `echo://` links and opens the same links in the window, previews a mail and a Teams message without sending them, changes a series' rule and two settings, adds a client and a partner organisation, makes a thread for Duff and shapes it, edits a thread's work, and tidies one call: its kind, its title, its action item, its minutes page, its trash and back.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run, built right before it and before 21:00 local (stages 3, 4 read a meeting still to come).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `cli/TC-007_help-lists-the-commands-and-read-commands-print-json.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `cli/TC-008_project-list-show-of-people-found-match-the-window.md` | — | — |
| 3 | `cli/TC-013_read-gives-the-text-behind-each-kind-of-echo-link.md` | — | — |
| 4 | `shell/TC-016_echo-links-open-their-place-in-the-window.md` | — | — |
| 5 | `cli/TC-016_m365-sends-nothing-without-yes.md` | — | — |
| 6 | `cli/TC-014_series-and-config-show-and-change-what-the-window-shows.md` | — | — |
| 7 | `cli/TC-015_client-and-org-commands-change-what-the-window-shows.md` | — | — |
| 8 | `cli/TC-009_project-create-set-rule-include-exclude-reset-delete.md` | — | — |
| 9 | `cli/TC-010_ledger-tree-and-edit-change-the-work-as-the-user.md` | — | — |
| 10 | `cli/TC-011_call-commands-context-actions-type-title-html.md` | — | — |
| 11 | `cli/TC-012_delete-restore-and-note-from-the-cli.md` | — | — |

## Notes
- The read stages (1–5) come before the ones that change data; each changing stage restores what it changed, except stage 9 (the work of Acme rollout: a criterion met, a move accepted, a merge rejected, a task made and deleted).
- Stage 9 step 12 and stage 10 step 13 are expected to FAIL until the suspected bugs they check are fixed (an unchecked `target` in ledger `accept`/`reject`/`undo`; "latest" picking an undated call); no stage depends on them.
- Every CLI step runs `target/debug/echoctl` after `. <dir>/env`; window steps after a CLI change reload the page first.
