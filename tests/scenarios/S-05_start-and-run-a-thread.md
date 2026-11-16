---
id: S-05
title: Start and run a thread for a client: create it, brief, rules, calls, rename, stage, close
run: agent
data: small
cases:
  - tests/cases/threads/TC-001_list-groups-by-client.md
  - tests/cases/threads/TC-017_found-for-you-empty.md
  - tests/cases/threads/TC-003_new-thread-from-plus.md
  - tests/cases/threads/TC-002_header-tabs-and-actions-menu.md
  - tests/cases/threads/TC-016_profile-meetings-and-sources-without-data.md
  - tests/cases/threads/TC-012_profile-organisation-counterpart.md
  - tests/cases/threads/TC-013_profile-calls-rules-add-remove.md
  - tests/cases/threads/TC-014_membership-is-computed-never-written.md
  - tests/cases/threads/TC-009_calls-tab-take-out-rule-call.md
  - tests/cases/threads/TC-008_calls-tab-pinned-call-unpin.md
  - tests/cases/threads/TC-004_new-thread-from-a-call.md
  - tests/cases/threads/TC-007_brief-edit-save-cancel.md
  - tests/cases/threads/TC-006_rename-with-the-cli.md
  - tests/cases/threads/TC-015_profile-since-notes-colour.md
  - tests/cases/threads/TC-010_stage-closes-the-thread.md
  - tests/cases/threads/TC-011_profile-client-change-and-new-client.md
  - tests/cases/threads/TC-005_delete-thread-keeps-calls.md
  - tests/cases/threads/TC-028_mail-source-limited-to-subjects.md
auto: [tests/auto/specs/S-05.spec.mjs]
---

# S-05: Start and run a thread for a client: create it, brief, rules, calls, rename, stage, close

**Story:** Sam starts a thread for Initech's hiring, writes what Globex pilot is for, shapes which calls belong to Acme rollout, puts a call in a new thread, renames and tunes a thread, changes its client and finally closes and deletes threads.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–17 |
| Acme rollout's Profile open | stage 5, step 1 | 6, 7 |
| Globex pilot's Brief open | stage 12, step 1 | 13 |
| Globex pilot's Profile open | stage 14, step 1 | 15, 16 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `threads/TC-001_list-groups-by-client.md` | — | — |
| 2 | `threads/TC-017_found-for-you-empty.md` | — | — |
| 3 | `threads/TC-003_new-thread-from-plus.md` | — | — |
| 4 | `threads/TC-002_header-tabs-and-actions-menu.md` | — | — |
| 5 | `threads/TC-016_profile-meetings-and-sources-without-data.md` | — | — |
| 6 | `threads/TC-012_profile-organisation-counterpart.md` | — | On stage 5's view (Acme rollout's Profile): skip the navigation of step 1; check its result on the snapshot |
| 7 | `threads/TC-013_profile-calls-rules-add-remove.md` | — | On stage 6's view (Acme rollout's Profile): skip the navigation of step 1; check its result on the snapshot |
| 8 | `threads/TC-014_membership-is-computed-never-written.md` | — | — |
| 9 | `threads/TC-009_calls-tab-take-out-rule-call.md` | — | — |
| 10 | `threads/TC-008_calls-tab-pinned-call-unpin.md` | — | — |
| 11 | `threads/TC-004_new-thread-from-a-call.md` | — | — |
| 12 | `threads/TC-007_brief-edit-save-cancel.md` | — | — |
| 13 | `threads/TC-006_rename-with-the-cli.md` | — | On stage 12's view (Globex pilot's Brief): skip the navigation of step 1; check its heading on the snapshot |
| 14 | `threads/TC-015_profile-since-notes-colour.md` | — | Reload `{{base_url}}/` (stage 13 renamed a thread with the CLI) |
| 15 | `threads/TC-010_stage-closes-the-thread.md` | — | On stage 14's view (Globex pilot's Profile): skip the navigation of step 1; check its result on the snapshot |
| 16 | `threads/TC-011_profile-client-change-and-new-client.md` | — | On stage 15's view (Globex pilot's Profile, Active again after its teardown): skip the navigation of step 1; check its result on the snapshot |
| 17 | `threads/TC-005_delete-thread-keeps-calls.md` | — | Reload `{{base_url}}/` (stage 16 changed a client and removed one with the CLI) |
| 18 | `threads/TC-028_mail-source-limited-to-subjects.md` | — | — |

## Notes
- No stage depends on another: each leaves the data as it found it (its teardown), so a failed stage never blocks the next.
- The stages are grouped by the view they work on, so a view is reached once for the stages that only read or edit it: Acme rollout's Profile (stages 5–7), Globex pilot's Brief (12–13) and Profile (14–16). Stages 9 and 10 navigate on purpose: a CLI write comes before their view. A stage that goes on from the one before skips only its navigation; if that view isn't there (an earlier stage failed), run the skipped step as written.
- Stage 3 creates and deletes `initech-hiring`; stage 11 creates and deletes `initech-planning`; stage 17 creates and deletes `scratch-thread`.
- The subject rule `planning` is added three times on purpose (stages 7, 8 and 9): each case checks a different way of adding it (Profile, CLI, Calls tab), and each removal is a check.
- Stage 9 (take out) comes before stage 10 (unpin) because 9 needs Acme weekly sync pinned and 10 pins it back through the CLI.
- Stage 15 closes Globex pilot and re-opens it in the window (no reload needed after it); stage 13 renames it and renames it back with the CLI, stage 16 changes its client and removes Northwind with the CLI: the stage after each of these reloads `{{base_url}}/`. Load it again if a request answers 403 (cookies are shared by every port of 127.0.0.1).
- Stage 14's teardown resets since, notes and colour with the CLI while the Profile stays open; stages 15 and 16 change other fields of it (each a field-level write), so the stale display of those three is harmless.
