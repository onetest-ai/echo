---
id: S-10
title: Inbox and prep on a quiet day
run: agent
data: small
cases:
  - tests/cases/inbox/TC-001_panel-opens-from-activity-bar.md
  - tests/cases/inbox/TC-002_empty-open-and-done.md
  - tests/cases/inbox/TC-003_attention-cli-json.md
  - tests/cases/prep/TC-001_coming-up-today-fold.md
  - tests/cases/prep/TC-002_meeting-opens-like-a-call.md
  - tests/cases/prep/TC-003_no-prep-yet-no-thread.md
  - tests/cases/prep/TC-004_thread-changes-the-prep-line.md
  - tests/cases/prep/TC-005_prep-upcoming-cli.md
---

# S-10: Inbox and prep on a quiet day

**Story:** Sam opens the Inbox and sees that nothing waits, checks the same in the CLI, then looks at the meetings still to come today: the folded list, a meeting's page, the missing prep, a thread added to a meeting, and the CLI's list of them.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `inbox/TC-001_panel-opens-from-activity-bar.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `inbox/TC-002_empty-open-and-done.md` | — | — |
| 3 | `inbox/TC-003_attention-cli-json.md` | — | — |
| 4 | `prep/TC-001_coming-up-today-fold.md` | — | — |
| 5 | `prep/TC-002_meeting-opens-like-a-call.md` | — | — |
| 6 | `prep/TC-003_no-prep-yet-no-thread.md` | — | — |
| 7 | `prep/TC-004_thread-changes-the-prep-line.md` | — | — |
| 8 | `prep/TC-005_prep-upcoming-cli.md` | — | — |

## Notes
- Only what `small` holds: an empty Inbox and meetings with no prep. Open items (badge count, groups, per-kind buttons, Working…, Done rows) and written preps (sections, Write it again, the Prep tab of a recorded call) wait for the `standard` data set, which must hold them (for example a failed minutes run, a pending people suggestion, a written prep).
- Run before 22:00 local: the prep stages need two meetings still to come today.
- No stage presses Prepare now or any Inbox button.
