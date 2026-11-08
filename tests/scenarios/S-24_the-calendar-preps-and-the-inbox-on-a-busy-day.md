---
id: S-24
title: The calendar, preps and the Inbox on a busy day
run: agent
data: standard
cases:
  - tests/cases/calendar/TC-012_block-style-follows-recorded-scheduled-and-timing.md
  - tests/cases/calendar/TC-016_overlapping-meetings-sit-side-by-side.md
  - tests/cases/calendar/TC-015_every-response-is-drawn-alike-without-microsoft-365.md
  - tests/cases/calendar/TC-013_card-offers-one-action-by-state.md
  - tests/cases/calendar/TC-018_series-card-shows-the-rule-for-every-meeting.md
  - tests/cases/calendar/TC-017_card-of-a-recorded-meeting-names-its-recording.md
  - tests/cases/calendar/TC-023_day-and-week-switch-keeps-the-card-open.md
  - tests/cases/prep/TC-012_the-calendar-card-offers-show-prep-prepare-or-nothing.md
  - tests/cases/prep/TC-006_coming-up-today-lists-the-meetings-still-to-come-and-leaves-out-unanswered-ones.md
  - tests/cases/prep/TC-007_a-prep-shows-every-section-with-its-text.md
  - tests/cases/prep/TC-008_more-than-ten-people-show-ten-new-first-and-and-n-more.md
  - tests/cases/prep/TC-009_sections-follow-what-the-prep-holds.md
  - tests/cases/prep/TC-014_echoctl-prep-lists-and-shows-preps-on-the-standard-day.md
  - tests/cases/calendar/TC-022_week-shows-calls-without-a-meeting-as-blocks-of-their-own.md
  - tests/cases/calendar/TC-019_series-record-choice-is-kept.md
  - tests/cases/calendar/TC-020_kind-chip-on-a-card-sets-the-type-for-the-series.md
  - tests/cases/calendar/TC-021_thread-chips-on-a-card-and-plus-thread.md
  - tests/cases/inbox/TC-004_badge-counts-open-items-and-warns-when-one-blocks.md
  - tests/cases/inbox/TC-005_chips-open-blocking-and-done.md
  - tests/cases/inbox/TC-006_items-are-grouped-by-client-with-the-blocking-one-first.md
  - tests/cases/inbox/TC-007_more-than-five-of-a-kind-fold-into-n-more-like-these.md
  - tests/cases/inbox/TC-008_each-kind-carries-its-own-buttons.md
  - tests/cases/inbox/TC-017_done-list-and-the-done-this-week-strip.md
  - tests/cases/inbox/TC-018_echoctl-attention-agrees-with-the-window-on-the-standard-day.md
  - tests/cases/inbox/TC-016_open-goes-to-the-place-of-the-item.md
  - tests/cases/inbox/TC-012_import-teams-transcript-without-microsoft-365-fails-and-says-why.md
  - tests/cases/calendar/TC-014_import-teams-transcript-fails-cleanly-without-microsoft-365.md
  - tests/cases/inbox/TC-013_not-this-on-a-teams-transcript-drops-the-item.md
  - tests/cases/inbox/TC-009_answering-no-to-a-people-question-takes-it-out-and-lists-it-as-done.md
  - tests/cases/inbox/TC-010_found-for-you-not-this-drops-the-item-and-lists-it-as-done.md
  - tests/cases/inbox/TC-014_write-again-on-failed-minutes-fails-at-once-and-keeps-the-item.md
  - tests/cases/inbox/TC-015_ignore-on-a-sender-takes-the-item-out.md
  - tests/cases/inbox/TC-011_found-for-you-create-makes-the-thread.md
  - tests/cases/today/TC-007_wont-do-asks-for-a-comment-and-cancel-keeps-the-item.md
  - tests/cases/today/TC-008_wont-do-without-a-request-notes-who-its-for-and-drafts-nothing.md
  - tests/cases/today/TC-009_wont-do-asking-for-a-reply-drafts-it-and-sends-nothing-before-send.md
  - tests/cases/today/TC-010_drop-leaves-nothing-of-a-wont-do-draft.md
  - tests/cases/calendar/TC-024_right-click-a-meeting-opens-its-menu.md
---

# S-24: The calendar, preps and the Inbox on a busy day

**Story:** Sam looks at a day of 37 meetings: the blocks of every state, overlapping meetings, the six answers to an invite, the cards' actions, a recurring meeting's rule, a recorded meeting's card, Day and Week, and the prep a card offers; then Coming up today with its preps (every section, more than ten people, the Prep tab of recorded calls) and the CLI; the week; then the series, kind and thread choices on the cards; then the Inbox with 25 items: the badge, chips, groups, folds and buttons, the Done list, the CLI and where Open goes; and finally the decisions that work without a network or Claude Code.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Standard data, built right before the run | `serve.sh up <dir> <port> --set standard` | all |
| The meetings' times (live until 20 minutes after the build, to come from 37 minutes after it) | the data | 1–13 (read within 15 minutes of the build) |
| Today's Day view | stage 1, step 1 | 2–8 (each opens its own cards; no stage closes a card the next one replaces) |
| Coming up today unfolded | stage 9, step 3 | 10 |
| Globex sponsor update's prep open | stage 10, step 1 | 11 |
| Umbrella intro call open on Prep | stage 11, step 5 | 12 |
| The Day view after stage 15's reload | stage 15, step 4 | 16, 17 |
| The Inbox panel open (it stays open across views) | stage 19, step 1 | 20–28 |
| The Inbox panel after a full load (the people fold closed again) | stage 29, step 1 | 30–33 |
| Acme design review's card with the import's message | stage 26, step 5 | 27 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calendar/TC-012_block-style-follows-recorded-scheduled-and-timing.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `calendar/TC-016_overlapping-meetings-sit-side-by-side.md` | — | On stage 1's view: skip step 1 |
| 3 | `calendar/TC-015_every-response-is-drawn-alike-without-microsoft-365.md` | — | Skip step 1. Skip its teardown (stage 4 opens another card) |
| 4 | `calendar/TC-013_card-offers-one-action-by-state.md` | — | In step 1 skip the navigation; click the block. Skip its teardown |
| 5 | `calendar/TC-018_series-card-shows-the-rule-for-every-meeting.md` | — | In step 1 skip the navigation. Skip its teardown |
| 6 | `calendar/TC-017_card-of-a-recorded-meeting-names-its-recording.md` | — | In step 1 skip the navigation |
| 7 | `calendar/TC-023_day-and-week-switch-keeps-the-card-open.md` | — | Skip step 1 (the Day view, no card) |
| 8 | `prep/TC-012_the-calendar-card-offers-show-prep-prepare-or-nothing.md` | — | On stage 7's view: in step 1 skip the navigation; click `[data-ev="ev-m20"]` |
| 9 | `prep/TC-006_coming-up-today-lists-the-meetings-still-to-come-and-leaves-out-unanswered-ones.md` | — | Skip its teardown: Coming up today stays unfolded for stage 10 |
| 10 | `prep/TC-007_a-prep-shows-every-section-with-its-text.md` | — | On stage 9's view: in step 1 skip the navigation and the click on "Coming up today" (it is unfolded); click its row "Globex sponsor update" |
| 11 | `prep/TC-008_more-than-ten-people-show-ten-new-first-and-and-n-more.md` | — | On stage 10's view: skip step 1 (the prep is open). Skip its teardown |
| 12 | `prep/TC-009_sections-follow-what-the-prep-holds.md` | — | On stage 11's view: in step 1 skip the navigation and the clicks (Umbrella intro call is open on Prep) |
| 13 | `prep/TC-014_echoctl-prep-lists-and-shows-preps-on-the-standard-day.md` | — | — |
| 14 | `calendar/TC-022_week-shows-calls-without-a-meeting-as-blocks-of-their-own.md` | — | — |
| 15 | `calendar/TC-019_series-record-choice-is-kept.md` | — | Skip its teardown: no later stage reads Globex weekly's record rule (stage 5 did) |
| 16 | `calendar/TC-020_kind-chip-on-a-card-sets-the-type-for-the-series.md` | — | On stage 15's view: in step 1 skip the navigation; click the block |
| 17 | `calendar/TC-021_thread-chips-on-a-card-and-plus-thread.md` | — | In step 1 skip the navigation; click the block |
| 18 | `inbox/TC-004_badge-counts-open-items-and-warns-when-one-blocks.md` | — | — (the server has run for minutes: the sign-in check has settled) |
| 19 | `inbox/TC-005_chips-open-blocking-and-done.md` | — | On stage 18's view: in step 1 skip the navigation; click "Inbox". Skip its teardown: the panel stays open |
| 20 | `inbox/TC-006_items-are-grouped-by-client-with-the-blocking-one-first.md` | — | On stage 19's view: skip step 1 (the Inbox is open on Open). Skip its teardown |
| 21 | `inbox/TC-007_more-than-five-of-a-kind-fold-into-n-more-like-these.md` | — | Skip step 1. Skip its teardown (the people stay unfolded until stage 29's reload) |
| 22 | `inbox/TC-008_each-kind-carries-its-own-buttons.md` | — | Skip step 1. Skip its teardown |
| 23 | `inbox/TC-017_done-list-and-the-done-this-week-strip.md` | — | Skip step 1. Teardown: click the chip "Open" only (the panel stays open) |
| 24 | `inbox/TC-018_echoctl-attention-agrees-with-the-window-on-the-standard-day.md` | — | In step 5 the panel is already open: read its chips. Skip its teardown |
| 25 | `inbox/TC-016_open-goes-to-the-place-of-the-item.md` | — | Skip step 1. Skip its teardown: the panel stays open for stage 26 |
| 26 | `inbox/TC-012_import-teams-transcript-without-microsoft-365-fails-and-says-why.md` | — | On stage 25's view: skip step 1 (the Inbox is open over Settings). Skip its teardown: the card stays open for stage 27 |
| 27 | `calendar/TC-014_import-teams-transcript-fails-cleanly-without-microsoft-365.md` | — | On stage 26's view: skip step 1 (the card of Acme design review is open; its line may already hold the message, it must still show after the click). Skip its teardown |
| 28 | `inbox/TC-013_not-this-on-a-teams-transcript-drops-the-item.md` | — | Skip step 1 (the Inbox is open). Skip its teardown |
| 29 | `inbox/TC-009_answering-no-to-a-people-question-takes-it-out-and-lists-it-as-done.md` | — | Reload `{{base_url}}/#today` (a full load: the fold stage 21 opened stays until a reload, and step 3 reads it); in step 1 click "Inbox" only if the panel is not open on Inbox. Skip its teardown |
| 30 | `inbox/TC-010_found-for-you-not-this-drops-the-item-and-lists-it-as-done.md` | — | In step 1 skip the navigation and the click (the Inbox is open); note N and D. Skip its teardown |
| 31 | `inbox/TC-014_write-again-on-failed-minutes-fails-at-once-and-keeps-the-item.md` | — | Skip step 1. Skip its teardown |
| 32 | `inbox/TC-015_ignore-on-a-sender-takes-the-item-out.md` | — | On stage 31's view: skip step 1 (the Inbox stays open over the call). Skip its teardown |
| 33 | `inbox/TC-011_found-for-you-create-makes-the-thread.md` | — | Skip step 1. **Skip its teardown** (the last stage: the thread Acme calls stays) |
| 34 | `today/TC-007_wont-do-asks-for-a-comment-and-cancel-keeps-the-item.md` | — | — |
| 35 | `today/TC-008_wont-do-without-a-request-notes-who-its-for-and-drafts-nothing.md` | — | — |
| 36 | `today/TC-009_wont-do-asking-for-a-reply-drafts-it-and-sends-nothing-before-send.md` | — | — |
| 37 | `today/TC-010_drop-leaves-nothing-of-a-wont-do-draft.md` | — | — |
| 38 | `calendar/TC-024_right-click-a-meeting-opens-its-menu.md` | — | — |

## Notes
- A stage that continues an earlier stage's view ("On stage N's view") takes one snapshot first; if the state is not there (the earlier stage failed before it), it does the skipped steps after all.
- Data: `standard`, built right before the run.
- Stages 1 to 13 read the clock: the meetings "live" at the build end 20 minutes after it and the ones "to come" start 37 minutes after it. Run these stages within 15 minutes of the build; if more than 20 minutes have passed, rebuild the data and start again from stage 1. `--now` does not help: the window places the meetings against the real clock, and `--now` only shifts the data.
- Stages 1 to 8 stay on the Day view: each opens its own cards, and a card is closed only where a step checks it (stages 6 and 7).
- Stages 15 to 17 change the calendar's own files (a series rule, a meeting's type, a pinned thread); stage 15 leaves Globex weekly on Record (no later stage reads it), stage 17 takes its own pin out again.
- Inbox stages 18 to 25 read first (badge, chips, groups, folds, buttons, Done rows, the CLI, Open): none of them takes a decision, so Done still holds the data's two rows. The panel opens once (stage 19) and stays open across views until the end. The decisions come last (26 to 33), one item each.
- The decision stages read the Inbox's counts at their start and expect one less Open and one more Done (stage 28 has already taken one item when 29 and 30 run). Stage 29 starts with a full load: the "5 more like these" that stage 21 opened stays open until a reload, and stage 29 reads the fold.
- Stage 26 (Inbox) presses Import without Microsoft 365: it fails with a message, and its Open lands on the meeting's card, where stage 27 (calendar) presses the card's own Import and reads the same message.
- `prep/TC-013` (Prepare now with the fake Claude Code) is not a stage: it is expected to expose a defect (see its Notes) and runs alone.
- Nothing here starts Claude Code except `inbox/TC-014` (Write again), which runs the data's fake and fails at once; no stage needs Microsoft 365.
