---
id: S-05
title: Start and run a thread for a client: create it, brief, rules, calls, rename, stage, close
run: agent
data: small
cases:
  - tests/cases/threads/TC-001_list-groups-by-client.md
  - tests/cases/threads/TC-002_header-tabs-and-actions-menu.md
  - tests/cases/threads/TC-017_found-for-you-empty.md
  - tests/cases/threads/TC-003_new-thread-from-plus.md
  - tests/cases/threads/TC-007_brief-edit-save-cancel.md
  - tests/cases/threads/TC-013_profile-calls-rules-add-remove.md
  - tests/cases/threads/TC-014_membership-is-computed-never-written.md
  - tests/cases/threads/TC-009_calls-tab-take-out-rule-call.md
  - tests/cases/threads/TC-008_calls-tab-pinned-call-unpin.md
  - tests/cases/threads/TC-004_new-thread-from-a-call.md
  - tests/cases/threads/TC-006_rename-with-the-cli.md
  - tests/cases/threads/TC-015_profile-since-notes-colour.md
  - tests/cases/threads/TC-016_profile-meetings-and-sources-without-data.md
  - tests/cases/threads/TC-011_profile-client-change-and-new-client.md
  - tests/cases/threads/TC-012_profile-organisation-counterpart.md
  - tests/cases/threads/TC-010_stage-closes-the-thread.md
  - tests/cases/threads/TC-005_delete-thread-keeps-calls.md
---

# S-05: Start and run a thread for a client: create it, brief, rules, calls, rename, stage, close

**Story:** Sam starts a thread for Initech's hiring, writes what Globex pilot is for, shapes which calls belong to Acme rollout, puts a call in a new thread, renames and tunes a thread, changes its client and finally closes and deletes threads.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `threads/TC-001_list-groups-by-client.md` | — | — |
| 2 | `threads/TC-002_header-tabs-and-actions-menu.md` | — | — |
| 3 | `threads/TC-017_found-for-you-empty.md` | — | — |
| 4 | `threads/TC-003_new-thread-from-plus.md` | — | — |
| 5 | `threads/TC-007_brief-edit-save-cancel.md` | — | — |
| 6 | `threads/TC-013_profile-calls-rules-add-remove.md` | — | — |
| 7 | `threads/TC-014_membership-is-computed-never-written.md` | — | — |
| 8 | `threads/TC-009_calls-tab-take-out-rule-call.md` | — | — |
| 9 | `threads/TC-008_calls-tab-pinned-call-unpin.md` | — | — |
| 10 | `threads/TC-004_new-thread-from-a-call.md` | — | — |
| 11 | `threads/TC-006_rename-with-the-cli.md` | — | — |
| 12 | `threads/TC-015_profile-since-notes-colour.md` | — | — |
| 13 | `threads/TC-016_profile-meetings-and-sources-without-data.md` | — | — |
| 14 | `threads/TC-011_profile-client-change-and-new-client.md` | — | — |
| 15 | `threads/TC-012_profile-organisation-counterpart.md` | — | — |
| 16 | `threads/TC-010_stage-closes-the-thread.md` | — | — |
| 17 | `threads/TC-005_delete-thread-keeps-calls.md` | — | — |

## Notes
- No stage depends on another: each leaves the data as it found it (its teardown), so a failed stage never blocks the next.
- Stage 4 creates and deletes `initech-hiring`; stage 10 creates and deletes `initech-planning`; stage 17 creates and deletes `scratch-thread`.
- Stage 8 (take out) comes before stage 9 (unpin) because 8 needs Acme weekly sync pinned and 9 pins it back through the CLI.
- Stage 16 closes Globex pilot and re-opens it; stage 11 renames it and renames it back.
- Reload `{{base_url}}/` before a stage that follows a client or rename change, and load it again if a request answers 403 (cookies are shared by every port of 127.0.0.1).
