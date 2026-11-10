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
  - tests/cases/cli/TC-017_read-inside-a-run-reads-the-calls-it-may-and-says-why-not.md
auto: [tests/auto/specs/S-28.spec.mjs]
---

# S-28: Echo from the terminal: help, threads, links, sends, series and settings, clients, thread changes, the work and one call

**Story:** Sam works from a terminal (or a Claude Code session does it for him) with the window open beside it. He looks up what `echoctl` can do, lists his threads and checks them against the window, reads what is behind a few `echo://` links and opens the same links in the window, previews a mail and a Teams message without sending them, changes a series' rule and two settings, adds a client and a partner organisation, makes a thread for Duff and shapes it, edits a thread's work, and tidies one call: its kind, its title, its action item, its minutes page, its trash and back.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run, built right before it and before 21:00 local (stages 3, 4 read a meeting still to come).

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`standard`) | the lead (`serve.sh up … --set standard`), right before the run and before 21:00 | all |
| `echoctl project list` on the data as built, saved to `<dir>/project-list.json` | Prelude, step P2 | 1 (step 6), 2 (step 1) |
| One shell call per run of consecutive CLI steps (the batches below) | each stage | each stage: a batch's output is checked step by step against each step's expected result |

## Prelude

Done once, before stage 1, timed as the stage `prelude`.

| # | Action | Expected Result |
|---|---|---|
| P1 | Load `{{base_url}}/` (the cookie) in the open browser | The window loads |
| P2 | `. <dir>/env; target/debug/echoctl project list > <dir>/project-list.json; echo "exit $?"` | `exit 0`. Nothing changes the data before stage 2 has read it |

## Stages

**Batches.** "Steps a–b in one call" means: run those steps' commands in **one** Bash call, in the steps' order, after `. <dir>/env` (and the case's own variable lookups, e.g. `A=$(ls …)`), each command preceded by `echo "== step N"` and followed by `echo "exit $?"`; then check every step's expected result against its part of the output. Only consecutive steps that run commands and nothing in the window are batched; a step that reads the window, reloads it or needs a value read from an earlier output (a `mid`) is never inside a batch. A step whose command comes from the prelude (`project list`) reads `<dir>/project-list.json` instead of running it again.

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `cli/TC-007_help-lists-the-commands-and-read-commands-print-json.md` | — | Steps 1–7 in one call; step 6's `project list` is `cat <dir>/project-list.json` |
| 2 | `cli/TC-008_project-list-show-of-people-found-match-the-window.md` | — | Step 1 reads `<dir>/project-list.json`. Before step 2, run the commands of steps 3, 5, 6, 8 and 10 in one call (all reads; nothing changes the data during this stage) and check each when the stage reaches it; the window steps 2, 4, 7, 9 and 11 run in their order |
| 3 | `cli/TC-013_read-gives-the-text-behind-each-kind-of-echo-link.md` | — | Steps 1–9 in one call |
| 4 | `shell/TC-016_echo-links-open-their-place-in-the-window.md` | — | — (window only: every step is its own full load) |
| 5 | `cli/TC-016_m365-sends-nothing-without-yes.md` | — | Steps 1–7 in one call |
| 6 | `cli/TC-014_series-and-config-show-and-change-what-the-window-shows.md` | — | Batches: steps 3–4, steps 6–7 (step 6 prints the settings before step 7 changes them). Step 10 is one call already |
| 7 | `cli/TC-015_client-and-org-commands-change-what-the-window-shows.md` | — | Batches: steps 1–4; the command of step 8, then the reload; steps 9–10 and step 11's `client list` in one call, then step 11's reload |
| 8 | `cli/TC-009_project-create-set-rule-include-exclude-reset-delete.md` | — | Batches: steps 1–2; steps 4–6; steps 8–11 |
| 9 | `cli/TC-010_ledger-tree-and-edit-change-the-work-as-the-user.md` | — | Step 1 alone (it gives the two `mid`s); then steps 2–6 in one call, each edit followed by `$E tree` piped through `python3 -c` that prints only the nodes the step names; step 8 alone (it gives the merge's `mid`); step 9 alone; step 10's edit, then its reload; steps 11–12 in one call |
| 10 | `cli/TC-011_call-commands-context-actions-type-title-html.md` | — | Batches: steps 1–7; step 9 alone (before the window reads the new name); steps 11–13 |
| 11 | `cli/TC-012_delete-restore-and-note-from-the-cli.md` | — | Batches: steps 2–3; steps 5–6; steps 8–9 |
| 12 | `cli/TC-017_read-inside-a-run-reads-the-calls-it-may-and-says-why-not.md` | — | Steps 1–6 in one call (read only; the calls it reads, Globex pilot scoping and the older Acme weekly sync, are ones stages 6–11 leave as built) |

## Notes
- The read stages (1–5) come before the ones that change data; each changing stage restores what it changed, except stage 9 (the work of Acme rollout: a criterion met, a move accepted, a merge rejected, a task made and deleted).
- `<dir>/project-list.json` is the data as built: stages 1 and 2 read it instead of running `project list` twice. Stages 6–11 change the data after it, so nothing later reads it.
- Batching changes no check: each step's expected result is still checked on its own part of the output, and a failed step fails the stage as before (record which `== step N` part failed). A batch never spans a window step, so every "reload after a CLI change" still sees the change.
- Every CLI step runs `target/debug/echoctl` after `. <dir>/env`; window steps after a CLI change reload the page first.
