---
id: S-24
title: The calendar, preps and the Inbox on a busy day
run: agent
data: standard
cases:
  - tests/cases/calendar/TC-012_block-style-follows-recorded-scheduled-and-timing.md
  - tests/cases/calendar/TC-013_card-offers-one-action-by-state.md
  - tests/cases/calendar/TC-015_every-response-is-drawn-alike-without-microsoft-365.md
  - tests/cases/calendar/TC-016_overlapping-meetings-sit-side-by-side.md
  - tests/cases/calendar/TC-017_card-of-a-recorded-meeting-names-its-recording.md
  - tests/cases/calendar/TC-018_series-card-shows-the-rule-for-every-meeting.md
  - tests/cases/calendar/TC-022_week-shows-calls-without-a-meeting-as-blocks-of-their-own.md
  - tests/cases/calendar/TC-023_day-and-week-switch-keeps-the-card-open.md
  - tests/cases/prep/TC-006_coming-up-today-lists-the-meetings-still-to-come-and-leaves-out-unanswered-ones.md
  - tests/cases/prep/TC-007_a-prep-shows-every-section-with-its-text.md
  - tests/cases/prep/TC-008_more-than-ten-people-show-ten-new-first-and-and-n-more.md
  - tests/cases/prep/TC-009_sections-follow-what-the-prep-holds.md
  - tests/cases/prep/TC-012_the-calendar-card-offers-show-prep-prepare-or-nothing.md
  - tests/cases/prep/TC-014_echoctl-prep-lists-and-shows-preps-on-the-standard-day.md
  - tests/cases/calendar/TC-019_series-record-choice-is-kept.md
  - tests/cases/calendar/TC-020_kind-chip-on-a-card-sets-the-type-for-the-series.md
  - tests/cases/calendar/TC-021_thread-chips-on-a-card-and-plus-thread.md
  - tests/cases/inbox/TC-004_badge-counts-open-items-and-warns-when-one-blocks.md
  - tests/cases/inbox/TC-005_chips-open-blocking-and-done.md
  - tests/cases/inbox/TC-006_items-are-grouped-by-client-with-the-blocking-one-first.md
  - tests/cases/inbox/TC-007_more-than-five-of-a-kind-fold-into-n-more-like-these.md
  - tests/cases/inbox/TC-008_each-kind-carries-its-own-buttons.md
  - tests/cases/inbox/TC-016_open-goes-to-the-place-of-the-item.md
  - tests/cases/inbox/TC-017_done-list-and-the-done-this-week-strip.md
  - tests/cases/inbox/TC-018_echoctl-attention-agrees-with-the-window-on-the-standard-day.md
  - tests/cases/calendar/TC-014_import-teams-transcript-fails-cleanly-without-microsoft-365.md
  - tests/cases/inbox/TC-012_import-teams-transcript-without-microsoft-365-fails-and-says-why.md
  - tests/cases/inbox/TC-013_not-this-on-a-teams-transcript-drops-the-item.md
  - tests/cases/inbox/TC-009_answering-no-to-a-people-question-takes-it-out-and-lists-it-as-done.md
  - tests/cases/inbox/TC-010_found-for-you-not-this-drops-the-item-and-lists-it-as-done.md
  - tests/cases/inbox/TC-011_found-for-you-create-makes-the-thread.md
  - tests/cases/inbox/TC-014_write-again-on-failed-minutes-fails-at-once-and-keeps-the-item.md
  - tests/cases/inbox/TC-015_ignore-on-a-sender-takes-the-item-out.md
  - tests/cases/today/TC-007_wont-do-asks-for-a-comment-and-cancel-keeps-the-item.md
  - tests/cases/today/TC-008_wont-do-without-a-request-notes-who-its-for-and-drafts-nothing.md
  - tests/cases/today/TC-009_wont-do-asking-for-a-reply-drafts-it-and-sends-nothing-before-send.md
  - tests/cases/today/TC-010_drop-leaves-nothing-of-a-wont-do-draft.md
  - tests/cases/calendar/TC-024_right-click-a-meeting-opens-its-menu.md
---

# S-24: The calendar, preps and the Inbox on a busy day

**Story:** Sam looks at a day of 37 meetings: the blocks and cards of every state, the six answers to an invite, overlapping meetings, a recorded meeting's card, a recurring meeting's rule and the week; then Coming up today with its preps (every section, more than ten people, the Prep tab of recorded calls), the series, kind and thread choices on the cards; then the Inbox with 25 items: the badge, chips, groups, folds and buttons, where Open goes, the Done list and the CLI; then the decisions that work without a network or Claude Code; and last, Won't do on Today's Needs you now: the comment it needs, Claude's note of who it's for, a draft that waits for Send, and Drop.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calendar/TC-012_block-style-follows-recorded-scheduled-and-timing.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `calendar/TC-013_card-offers-one-action-by-state.md` | — | — |
| 3 | `calendar/TC-015_every-response-is-drawn-alike-without-microsoft-365.md` | — | — |
| 4 | `calendar/TC-016_overlapping-meetings-sit-side-by-side.md` | — | — |
| 5 | `calendar/TC-017_card-of-a-recorded-meeting-names-its-recording.md` | — | — |
| 6 | `calendar/TC-018_series-card-shows-the-rule-for-every-meeting.md` | — | — |
| 7 | `calendar/TC-022_week-shows-calls-without-a-meeting-as-blocks-of-their-own.md` | — | — |
| 8 | `calendar/TC-023_day-and-week-switch-keeps-the-card-open.md` | — | — |
| 9 | `prep/TC-006_coming-up-today-lists-the-meetings-still-to-come-and-leaves-out-unanswered-ones.md` | — | — |
| 10 | `prep/TC-007_a-prep-shows-every-section-with-its-text.md` | — | — |
| 11 | `prep/TC-008_more-than-ten-people-show-ten-new-first-and-and-n-more.md` | — | — |
| 12 | `prep/TC-009_sections-follow-what-the-prep-holds.md` | — | — |
| 13 | `prep/TC-012_the-calendar-card-offers-show-prep-prepare-or-nothing.md` | — | — |
| 14 | `prep/TC-014_echoctl-prep-lists-and-shows-preps-on-the-standard-day.md` | — | — |
| 15 | `calendar/TC-019_series-record-choice-is-kept.md` | — | — |
| 16 | `calendar/TC-020_kind-chip-on-a-card-sets-the-type-for-the-series.md` | — | — |
| 17 | `calendar/TC-021_thread-chips-on-a-card-and-plus-thread.md` | — | — |
| 18 | `inbox/TC-004_badge-counts-open-items-and-warns-when-one-blocks.md` | — | Wait 10 seconds so the sign-in check has settled |
| 19 | `inbox/TC-005_chips-open-blocking-and-done.md` | — | — |
| 20 | `inbox/TC-006_items-are-grouped-by-client-with-the-blocking-one-first.md` | — | — |
| 21 | `inbox/TC-007_more-than-five-of-a-kind-fold-into-n-more-like-these.md` | — | — |
| 22 | `inbox/TC-008_each-kind-carries-its-own-buttons.md` | — | — |
| 23 | `inbox/TC-016_open-goes-to-the-place-of-the-item.md` | — | — |
| 24 | `inbox/TC-017_done-list-and-the-done-this-week-strip.md` | — | — |
| 25 | `inbox/TC-018_echoctl-attention-agrees-with-the-window-on-the-standard-day.md` | — | — |
| 26 | `calendar/TC-014_import-teams-transcript-fails-cleanly-without-microsoft-365.md` | — | — |
| 27 | `inbox/TC-012_import-teams-transcript-without-microsoft-365-fails-and-says-why.md` | — | — |
| 28 | `inbox/TC-013_not-this-on-a-teams-transcript-drops-the-item.md` | — | — |
| 29 | `inbox/TC-009_answering-no-to-a-people-question-takes-it-out-and-lists-it-as-done.md` | — | Reload `{{base_url}}/#today` (stage 21 unfolded the people questions; a reload folds them again) |
| 30 | `inbox/TC-010_found-for-you-not-this-drops-the-item-and-lists-it-as-done.md` | — | — |
| 31 | `inbox/TC-011_found-for-you-create-makes-the-thread.md` | — | — |
| 32 | `inbox/TC-014_write-again-on-failed-minutes-fails-at-once-and-keeps-the-item.md` | — | — |
| 33 | `inbox/TC-015_ignore-on-a-sender-takes-the-item-out.md` | — | — |
| 34 | `today/TC-007_wont-do-asks-for-a-comment-and-cancel-keeps-the-item.md` | — | — |
| 35 | `today/TC-008_wont-do-without-a-request-notes-who-its-for-and-drafts-nothing.md` | — | — |
| 36 | `today/TC-009_wont-do-asking-for-a-reply-drafts-it-and-sends-nothing-before-send.md` | — | — |
| 37 | `today/TC-010_drop-leaves-nothing-of-a-wont-do-draft.md` | — | — |
| 38 | `calendar/TC-024_right-click-a-meeting-opens-its-menu.md` | — | — |

## Notes
- Stages 34–37 are Today → Needs you now's Won't do: each decides one of the data's three items (none comes back), so they run once per data set, in this order.
- Data: `standard`, built right before the run.
- Stages 1 to 14 read the clock: the meetings "live" at the build end 20 minutes after it and the ones "to come" start 37 minutes after it. Run these stages within 15 minutes of the build; if more than 20 minutes have passed, rebuild the data and start again from stage 1.
- Stages 15 to 17 change the calendar's own files (a series rule, a meeting's type, a pinned thread) and put back what they can.
- Inbox stages 18 to 25 read first (badge, chips, groups, folds, buttons, Open, Done rows, the CLI): none of them takes a decision, so Done still holds the data's two rows. The decisions come last (26 to 33), one item each.
- Stage 18's step 1 fails until #115 is fixed (no badge after a full load on Today until the panel opens).
- The decision stages read the Inbox's counts at their start and expect one less Open and one more Done (stage 28 has
  already taken one item when 29 and 30 run).
- Stage 26 (calendar) presses Import on a meeting card without Microsoft 365: it fails with a message and the Inbox stage 27 reads the same message after its own click.
- `prep/TC-013` (Prepare now with the fake Claude Code) is not a stage: it is expected to expose a defect (see its Notes) and runs alone.
- Nothing here starts Claude Code except `inbox/TC-014` (Write again), which runs the data's fake and fails at once; no stage needs Microsoft 365.
