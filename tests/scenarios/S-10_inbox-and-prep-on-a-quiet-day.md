---
id: S-10
title: Inbox and prep on a quiet day
run: agent
data: small
cases:
  - tests/cases/inbox/TC-003_attention-cli-json.md
  - tests/cases/inbox/TC-002_empty-open-and-done.md
  - tests/cases/inbox/TC-001_panel-opens-from-the-top-bar.md
  - tests/cases/prep/TC-001_coming-up-today-fold.md
  - tests/cases/prep/TC-002_meeting-opens-like-a-call.md
  - tests/cases/prep/TC-004_thread-changes-the-prep-line.md
  - tests/cases/prep/TC-003_no-prep-yet-no-thread.md
  - tests/cases/prep/TC-005_prep-upcoming-cli.md
  - tests/cases/today/TC-006_a-day-with-nothing-says-a-quiet-day.md
---

# S-10: Inbox and prep on a quiet day

**Story:** Sam opens the Inbox and sees that nothing waits, checks the same in the CLI, then looks at the meetings still to come today: the folded list, a meeting's page, the missing prep, a thread added to a meeting, and the CLI's list of them. Then the day empties (the meetings cancelled, the calls put away, the task done) and Today says it is a quiet day.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build (by 22:50) | the lead (`serve.sh up`) | 1–9 |
| The Inbox panel open on `#today` | stage 1, step 3 | 2 |
| `#today` with the panel closed | stage 2's Teardown | 3 |
| Coming up today unfolded in the Calls list | stage 4, step 3 | 5, 6, 7, 8 |
| Globex design review open on Prep | stage 5, step 1 | 6 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `inbox/TC-003_attention-cli-json.md` | — | Skip its Teardown (keep the Inbox panel open on `#today` for stage 2) |
| 2 | `inbox/TC-002_empty-open-and-done.md` | — | On stage 1's view (`#today`, the Inbox panel open): skip step 1; check its result on the snapshot. Its Teardown closes the panel |
| 3 | `inbox/TC-001_panel-opens-from-the-top-bar.md` | — | On stage 2's view (`#today`, the panel closed): skip the navigation of step 1; check its result on the snapshot |
| 4 | `prep/TC-001_coming-up-today-fold.md` | — | On stage 3's view (`#calls`, the panel closed): skip the navigation of step 1; check its result on the snapshot. Skip its Teardown's fold (keep Coming up today unfolded for stage 5) |
| 5 | `prep/TC-002_meeting-opens-like-a-call.md` | — | On stage 4's view (`#calls`, Coming up today unfolded): in step 1 skip the navigation and the click on "Coming up today", click "Globex design review". Skip its Teardown's fold (keep Coming up today unfolded for stage 6) |
| 6 | `prep/TC-004_thread-changes-the-prep-line.md` | — | On stage 5's view (Globex design review open on Prep): skip step 1; check its result on the snapshot. Skip the fold in its Teardown (keep it for stage 7); remove the chip if it is still there |
| 7 | `prep/TC-003_no-prep-yet-no-thread.md` | — | On stage 6's view (`#upcoming/…`, Coming up today unfolded): in step 1 skip the navigation and the click on "Coming up today", click "Acme demo". Skip its Teardown's fold (keep Coming up today unfolded for stage 8) |
| 8 | `prep/TC-005_prep-upcoming-cli.md` | — | Steps 1–2 (CLI) as written; in step 3, on stage 7's view (Coming up today unfolded): skip the navigation and the click, read the two rows. Its Teardown folds Coming up today |
| 9 | `today/TC-006_a-day-with-nothing-says-a-quiet-day.md` | — | — (last: it empties the agenda) |

## Notes
- Only what `small` holds: an empty Inbox and meetings with no prep. Open items (badge count, groups, per-kind buttons, Working…, Done rows) and written preps (sections, Write it again, the Prep tab of a recorded call) wait for the `standard` data set, which must hold them (for example a failed minutes run, a pending people suggestion, a written prep).
- The Inbox panel is opened once for stages 1 and 2 (stage 1 opens it, stage 2 reads it and closes it); stage 3 opens and closes it as its own check and ends on `#calls`, where the prep stages go on. Coming up today is unfolded once (stage 4) and folded once (stage 8's Teardown); stages 5 and 6 read the same meeting. If a view a stage continues from isn't there (an earlier stage failed), run the skipped steps as written.
- Build by 22:50 local: the prep stages need two meetings still to come today (after about 20:20 the later ones come sooner).
- No stage presses Prepare now or any Inbox button.
- Stage 9 empties the day with the CLI (agenda cleared, today's calls deleted, the task closed) and restores the calls and the task; the agenda stays empty, so it runs last.
