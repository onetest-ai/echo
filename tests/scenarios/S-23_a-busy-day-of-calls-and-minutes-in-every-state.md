---
id: S-23
title: A busy day of calls and minutes in every state
run: agent
data: standard
cases:
  - tests/cases/calls/TC-012_row-icons-in-every-status.md
  - tests/cases/calls/TC-013_clock-chip-marks-minutes-ahead.md
  - tests/cases/calls/TC-027_countdown-for-automatic-minutes.md
  - tests/cases/calls/TC-028_not-yet-until-i-say-holds-the-minutes-and-resumes.md
  - tests/cases/calls/TC-029_not-yet-15-minutes-moves-the-time-and-says-so.md
  - tests/cases/calls/TC-014_header-of-a-memo-and-of-a-teams-import.md
  - tests/cases/calls/TC-016_chat-and-screens-tabs-follow-what-the-call-holds.md
  - tests/cases/calls/TC-017_chat-tab-lists-messages.md
  - tests/cases/calls/TC-018_screens-tab-opens-a-frame-large.md
  - tests/cases/calls/TC-019_kept-live-notes-are-in-my-notes-not-the-transcript-tab.md
  - tests/cases/calls/TC-020_what-moved-lists-changes-and-marks.md
  - tests/cases/calls/TC-021_what-moved-says-why-a-thread-has-not-read-the-call.md
  - tests/cases/calls/TC-022_minutes-frame-shows-the-sections-each-call-holds.md
  - tests/cases/calls/TC-023_failed-run-over-existing-minutes-is-a-bar.md
  - tests/cases/calls/TC-025_a-run-without-minutes-failed-or-skipped-explains-itself.md
  - tests/cases/calls/TC-026_a-run-needing-sign-in-says-signed-out-or-ready-to-retry.md
  - tests/cases/calls/TC-030_held-and-thin-calls-say-why-no-minutes-come-by-themselves.md
  - tests/cases/calls/TC-031_minutes-due-now-say-starting-the-minutes.md
  - tests/cases/calls/TC-032_a-transcribed-call-echo-leaves-alone-offers-write-minutes-with-model-and-command.md
  - tests/cases/calls/TC-033_minutes-come-after-the-call-while-it-is-transcribed.md
  - tests/cases/calls/TC-034_a-call-without-a-transcript-offers-write-minutes-and-says-no-transcript.md
  - tests/cases/prep/TC-010_a-recorded-call-whose-meeting-had-a-prep-has-a-prep-tab-first.md
  - tests/cases/prep/TC-011_a-prep-with-only-an-error-says-so-and-has-no-in-short.md
  - tests/cases/calls/TC-015_memo-threads-with-spans-and-x.md
  - tests/cases/calls/TC-024_try-again-fails-at-once-and-says-why.md
  - tests/cases/calls/TC-035_delete-asks-first-and-moves-the-folder-to-the-trash.md
  - tests/cases/calls/TC-036_restore-brings-a-deleted-call-back.md
  - tests/cases/window/TC-023_my-notes-holds-the-live-notes-in-one-timeline.md
---

# S-23: A busy day of calls and minutes in every state

**Story:** Sam opens a day with 56 calls in every state and goes through them: the list's icons and clock, the countdown and Not yet, the headers of a memo and a Teams import, the tabs, chat, screens and kept live notes, what moved, the minutes frames, a failed run over minutes and without, a run waiting for a sign-in, the calls Echo leaves alone, a call being transcribed and one that failed or has audio only, the prep tabs, a memo's threads, Try again against the fake Claude Code, and finally a deleted and a restored call.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calls/TC-012_row-icons-in-every-status.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `calls/TC-013_clock-chip-marks-minutes-ahead.md` | — | — |
| 3 | `calls/TC-027_countdown-for-automatic-minutes.md` | — | — |
| 4 | `calls/TC-028_not-yet-until-i-say-holds-the-minutes-and-resumes.md` | — | — |
| 5 | `calls/TC-029_not-yet-15-minutes-moves-the-time-and-says-so.md` | — | — |
| 6 | `calls/TC-014_header-of-a-memo-and-of-a-teams-import.md` | — | — |
| 7 | `calls/TC-016_chat-and-screens-tabs-follow-what-the-call-holds.md` | — | — |
| 8 | `calls/TC-017_chat-tab-lists-messages.md` | — | — |
| 9 | `calls/TC-018_screens-tab-opens-a-frame-large.md` | — | — |
| 10 | `calls/TC-019_kept-live-notes-are-in-my-notes-not-the-transcript-tab.md` | — | — |
| 11 | `calls/TC-020_what-moved-lists-changes-and-marks.md` | — | — |
| 12 | `calls/TC-021_what-moved-says-why-a-thread-has-not-read-the-call.md` | — | — |
| 13 | `calls/TC-022_minutes-frame-shows-the-sections-each-call-holds.md` | — | — |
| 14 | `calls/TC-023_failed-run-over-existing-minutes-is-a-bar.md` | — | — |
| 15 | `calls/TC-025_a-run-without-minutes-failed-or-skipped-explains-itself.md` | — | — |
| 16 | `calls/TC-026_a-run-needing-sign-in-says-signed-out-or-ready-to-retry.md` | — | — |
| 17 | `calls/TC-030_held-and-thin-calls-say-why-no-minutes-come-by-themselves.md` | — | — |
| 18 | `calls/TC-031_minutes-due-now-say-starting-the-minutes.md` | — | — |
| 19 | `calls/TC-032_a-transcribed-call-echo-leaves-alone-offers-write-minutes-with-model-and-command.md` | — | — |
| 20 | `calls/TC-033_minutes-come-after-the-call-while-it-is-transcribed.md` | — | — |
| 21 | `calls/TC-034_a-call-without-a-transcript-offers-write-minutes-and-says-no-transcript.md` | — | — |
| 22 | `prep/TC-010_a-recorded-call-whose-meeting-had-a-prep-has-a-prep-tab-first.md` | — | — |
| 23 | `prep/TC-011_a-prep-with-only-an-error-says-so-and-has-no-in-short.md` | — | — |
| 24 | `calls/TC-015_memo-threads-with-spans-and-x.md` | — | — |
| 25 | `calls/TC-024_try-again-fails-at-once-and-says-why.md` | — | — |
| 26 | `calls/TC-035_delete-asks-first-and-moves-the-folder-to-the-trash.md` | — | — |
| 27 | `calls/TC-036_restore-brings-a-deleted-call-back.md` | — | — |
| 28 | `window/TC-023_my-notes-holds-the-live-notes-in-one-timeline.md` | — | — |

## Notes
- Data: `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`), built right before the run.
- Time-sensitive stages come first: automatic minutes come 15 minutes after the build (stages 2 to 5 read the countdown and Not yet; stages 4 and 5 put it back). Finish those within 15 minutes of the build.
- Stages that change data come last, after the stages that read what they change: the memo's thread × (stage 24) after the memo's headers and what moved (stages 6 and 12); Try again (stage 25) after the stages that read the first failure messages (14 and 15); Delete and restore last.
- Stage 20 (`calls/TC-033`) starts with a full reload: the window opens a call being transcribed on Transcript once per
  load, and stage 7 has opened Quick check-in before. Its step 2 fails while the URL lags the tab after that choice (the
  hash reads `/minutes` while Transcript is shown; no case asserts the hash, see the case's Notes). Stage 25's step 5 fails until #121 is fixed.
- Stage 22 and 23 are the `prep` cases about calls (Prep tab first, a prep with only an error).
- Stage 28 (My notes with the live notes) only reads Acme weekly sync, Initech planning and Quick check-in, which the stages before it leave as built; it opens My notes and leaves it open.
- No stage presses Write minutes, Write now, Sync now or Sign in…; Try again runs the data's fake Claude Code, which fails at once.
- Not in the data (`tests/fixtures/README.md` → Skipped): a call being recorded, a minutes run queued or running, Today's minutes states being reworked, a call held by a Claude Code session (`held_by`), a missing or unreadable pinned AI profile.
