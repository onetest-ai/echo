---
id: S-23
title: A busy day of calls and minutes in every state
run: agent
data: standard
cases:
  - tests/cases/calls/TC-012_row-icons-in-every-status.md
  - tests/cases/calls/TC-013_clock-chip-marks-minutes-ahead.md
  - tests/cases/calls/TC-027_countdown-for-automatic-minutes.md
  - tests/cases/calls/TC-029_not-yet-15-minutes-moves-the-time-and-says-so.md
  - tests/cases/calls/TC-028_not-yet-until-i-say-holds-the-minutes-and-resumes.md
  - tests/cases/prep/TC-011_a-prep-with-only-an-error-says-so-and-has-no-in-short.md
  - tests/cases/prep/TC-010_a-recorded-call-whose-meeting-had-a-prep-has-a-prep-tab-first.md
  - tests/cases/calls/TC-023_failed-run-over-existing-minutes-is-a-bar.md
  - tests/cases/calls/TC-022_minutes-frame-shows-the-sections-each-call-holds.md
  - tests/cases/calls/TC-025_a-run-without-minutes-failed-or-skipped-explains-itself.md
  - tests/cases/calls/TC-026_a-run-needing-sign-in-says-signed-out-or-ready-to-retry.md
  - tests/cases/calls/TC-030_held-and-thin-calls-say-why-no-minutes-come-by-themselves.md
  - tests/cases/calls/TC-032_a-transcribed-call-echo-leaves-alone-offers-write-minutes-with-model-and-command.md
  - tests/cases/calls/TC-031_minutes-due-now-say-starting-the-minutes.md
  - tests/cases/calls/TC-034_a-call-without-a-transcript-says-why-and-offers-no-write-minutes.md
  - tests/cases/calls/TC-014_header-of-a-memo-and-of-a-teams-import.md
  - tests/cases/calls/TC-016_chat-and-screens-tabs-follow-what-the-call-holds.md
  - tests/cases/calls/TC-033_minutes-come-after-the-call-while-it-is-transcribed.md
  - tests/cases/calls/TC-039_what-moved-is-a-right-panel-view-and-the-minutes-take-the-whole-tab.md
  - tests/cases/calls/TC-020_what-moved-lists-changes-and-marks.md
  - tests/cases/calls/TC-021_what-moved-says-why-a-thread-has-not-read-the-call.md
  - tests/cases/calls/TC-017_chat-tab-lists-messages.md
  - tests/cases/calls/TC-018_screens-tab-opens-a-frame-large.md
  - tests/cases/calls/TC-019_kept-live-notes-are-in-my-notes-not-the-transcript-tab.md
  - tests/cases/window/TC-023_my-notes-holds-the-live-notes-in-one-timeline.md
  - tests/cases/calls/TC-015_memo-threads-with-spans-and-x.md
  - tests/cases/calls/TC-024_try-again-fails-at-once-and-says-why.md
  - tests/cases/calls/TC-035_delete-asks-first-and-moves-the-folder-to-the-trash.md
  - tests/cases/calls/TC-036_restore-brings-a-deleted-call-back.md
  - tests/cases/calls/TC-042_a-recorded-calls-people-are-who-joined-not-who-was-invited.md
  - tests/cases/calls/TC-040_a-recording-call-with-live-notes-on-shows-the-live-transcript-alone-kept-at-its-end.md
  - tests/cases/calls/TC-041_with-live-notes-off-a-recording-call-says-how-to-turn-them-on-and-sends-nothing-to-a-model.md
auto: [tests/auto/specs/S-23.spec.mjs]
---

# S-23: A busy day of calls and minutes in every state

**Story:** Sam opens a day with 56 calls in every state and goes through them: the list's icons and clock, the countdown and Not yet, the prep tabs, the minutes frames, a failed run over minutes and without, a run waiting for a sign-in, the calls Echo leaves alone, one due now, one that failed or has audio only, the headers of a memo and a Teams import, the tabs and a call being transcribed, what moved, chat, screens and kept live notes; then a memo's threads, Try again against the fake Claude Code, and finally a deleted and a restored call.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Standard data, built right before the run | `serve.sh up <dir> <port> --set standard` | all |
| The Calls list | stage 1, step 1 (the only navigation to `#calls`) | 2–29: every later stage clicks its rows; only stage 25 opens a call by URL, and stage 29 reloads (a check) |
| Globex standup's countdown ("Minutes in N min", 15 minutes after the build) | the data | 2–5 |
| Globex standup moved by "15 minutes more" | stage 4, step 2 | 5: its steps 3–5 ("Until I say", then "Write them automatically again") are stage 4's teardown and check that it is undone |
| Globex standup open | stage 3, step 1 | 4, 5, 6 |
| The Minutes tab as the open tab | stage 3, step 1 | 7–17: each stage clicks its own rows, the tab stays (stage 15 goes to Transcript and back) |
| Acme weekly sync open on Minutes | stage 7, step 4 | 8 |
| Acme weekly sync open | stage 16, step 4 | 17 |
| Quick check-in opened for the first time since the page load, from the Minutes tab | stage 17, step 5 | 18 (instead of a full reload) |
| Acme weekly sync open on Screens | stage 23 | 24 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calls/TC-012_row-icons-in-every-status.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `calls/TC-013_clock-chip-marks-minutes-ahead.md` | — | On stage 1's view: skip step 1 |
| 3 | `calls/TC-027_countdown-for-automatic-minutes.md` | — | On stage 2's view: in step 1 skip the navigation; click the row Globex standup, then the tab "Minutes" |
| 4 | `calls/TC-029_not-yet-15-minutes-moves-the-time-and-says-so.md` | — | On stage 3's view: in step 1 skip the navigation and the clicks (Globex standup is open on Minutes); read the heading and note T0. **Skip its teardown**: stage 5 does it and checks it |
| 5 | `calls/TC-028_not-yet-until-i-say-holds-the-minutes-and-resumes.md` | — | On stage 4's view: in step 1 skip the navigation and the clicks; the heading reads "Minutes in N min (HH:MM)" (about 15 more than in stage 3) |
| 6 | `prep/TC-011_a-prep-with-only-an-error-says-so-and-has-no-in-short.md` | — | On stage 5's view: in step 1 skip the navigation and the row click (Globex standup is open); click the tab "Prep" |
| 7 | `prep/TC-010_a-recorded-call-whose-meeting-had-a-prep-has-a-prep-tab-first.md` | — | In step 1 skip the navigation (the list is shown) |
| 8 | `calls/TC-023_failed-run-over-existing-minutes-is-a-bar.md` | — | On stage 7's view: in step 1 skip the navigation and the clicks (Acme weekly sync is open on Minutes) |
| 9 | `calls/TC-022_minutes-frame-shows-the-sections-each-call-holds.md` | — | In step 1 skip the navigation; click the row Acme weekly sync (the Minutes tab stays) |
| 10 | `calls/TC-025_a-run-without-minutes-failed-or-skipped-explains-itself.md` | — | In step 1 skip the navigation; click the row (the Minutes tab stays) |
| 11 | `calls/TC-026_a-run-needing-sign-in-says-signed-out-or-ready-to-retry.md` | — | The same as stage 10 (the server has run for minutes: no wait) |
| 12 | `calls/TC-030_held-and-thin-calls-say-why-no-minutes-come-by-themselves.md` | — | The same as stage 10 |
| 13 | `calls/TC-032_a-transcribed-call-echo-leaves-alone-offers-write-minutes-with-model-and-command.md` | — | The same as stage 10 |
| 14 | `calls/TC-031_minutes-due-now-say-starting-the-minutes.md` | — | The same as stage 10 |
| 15 | `calls/TC-034_a-call-without-a-transcript-says-why-and-offers-no-write-minutes.md` | — | The same as stage 10 |
| 16 | `calls/TC-014_header-of-a-memo-and-of-a-teams-import.md` | — | Skip step 1 (the list is shown) |
| 17 | `calls/TC-016_chat-and-screens-tabs-follow-what-the-call-holds.md` | — | On stage 16's view: in step 1 skip the navigation and the click (Acme weekly sync is open; the open tab is Minutes) |
| 18 | `calls/TC-033_minutes-come-after-the-call-while-it-is-transcribed.md` | — | On stage 17's view: in step 1 skip the full load, the navigation and the click (Quick check-in was opened for the first time since the page load, from Minutes); check step 1's expected result |
| 19 | `calls/TC-039_what-moved-is-a-right-panel-view-and-the-minutes-take-the-whole-tab.md` | — | In step 1 skip the navigation; click the row Acme weekly sync |
| 20 | `calls/TC-020_what-moved-lists-changes-and-marks.md` | — | In step 1 skip the navigation |
| 21 | `calls/TC-021_what-moved-says-why-a-thread-has-not-read-the-call.md` | — | On stage 20's view: in step 1 skip the navigation and the What moved click (the panel shows it); click the row Memo |
| 22 | `calls/TC-017_chat-tab-lists-messages.md` | — | In step 1 skip the navigation |
| 23 | `calls/TC-018_screens-tab-opens-a-frame-large.md` | — | In step 1 skip the navigation |
| 24 | `calls/TC-019_kept-live-notes-are-in-my-notes-not-the-transcript-tab.md` | — | On stage 23's view: in step 1 skip the navigation and the row click (Acme weekly sync is open); click the tab "Transcript" |
| 25 | `window/TC-023_my-notes-holds-the-live-notes-in-one-timeline.md` | — | — (My notes is open from stage 24: leave it open) |
| 26 | `calls/TC-015_memo-threads-with-spans-and-x.md` | — | In step 1 skip the navigation. **Skip its teardown**: no later stage reads the memo's threads |
| 27 | `calls/TC-024_try-again-fails-at-once-and-says-why.md` | — | In step 1 skip the navigation |
| 28 | `calls/TC-035_delete-asks-first-and-moves-the-folder-to-the-trash.md` | — | In step 1 skip the navigation. Teardown: run the `echoctl restore` only, skip its reload (stage 29 needs the trash to hold one folder, not the list) |
| 29 | `calls/TC-036_restore-brings-a-deleted-call-back.md` | — | In step 1 skip the navigation (the list is shown). **Skip its teardown** (the last stage) |
| 30 | `calls/TC-042_a-recorded-calls-people-are-who-joined-not-who-was-invited.md` | — | Skip step 1 (stage 1's list already recorded who joined: `participants` is there) and step 2's navigation (the list is shown) |
| 31 | `calls/TC-040_a-recording-call-with-live-notes-on-shows-the-live-transcript-alone-kept-at-its-end.md` | — | — |
| 32 | `calls/TC-041_with-live-notes-off-a-recording-call-says-how-to-turn-them-on-and-sends-nothing-to-a-model.md` | — | — |

## Notes
- A stage that continues an earlier stage's view ("On stage N's view") takes one snapshot first; if the state is not there (the earlier stage failed before it), it does the skipped steps after all.
- Data: `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`), built right before the run.
- Time-sensitive stages come first: automatic minutes come 15 minutes after the build (stages 2 to 5 read the countdown and Not yet). Finish those within 15 minutes of the build. Neither `--now` nor `--clock fast` helps: the window counts down on the real clock, `--now` only shifts the data, and the fast clock makes the minutes due after 90 s.
- Stages 4 and 5 move Globex standup's minutes and put them back: "15 minutes more" (stage 4), then "Until I say" and "Write them automatically again" (stage 5's steps 3–5, which are also stage 4's teardown) bring back the time the user's notes set (`notes::set_later` keeps one choice: "as before" undoes either).
- Stages 6 to 15 run on the Minutes tab of one call after another; nothing leaves it but stage 15, which goes to Transcript and back. Stage 17's step 5 must open Quick check-in for the first time since the page load, with Minutes as the open tab: that is what stage 18 checks (#148) instead of its full load. No stage before 17 opens Quick check-in.
- Stages that change data come last, after the stages that read what they change: the memo's thread × (stage 26) after the memo's headers, tabs and what moved (stages 9, 16, 17, 19, 21); Try again (stage 27) after the stages that read the first failure messages (8 and 10); Delete and restore last; stage 30 only reads the calls.
- Stage 25 (My notes with the live notes) only reads Acme weekly sync, Initech planning and Quick check-in, which the stages before it leave as built; it opens Acme weekly sync by URL and leaves My notes open. Stage 19 (What moved in the right panel) reads Acme weekly sync, the Memo and Initech planning before stage 26 takes the memo out of Globex pilot.
- Stages 7 and 6 are the `prep` cases about calls (Prep tab first, a prep with only an error).
- No stage presses Write minutes, Write now, Sync now or Sign in…; Try again runs the data's fake Claude Code, which fails at once.
- Not in the data (`tests/fixtures/README.md` → Skipped): a call being recorded, a minutes run queued or running, Today's minutes states being reworked, a call held by a Claude Code session (`held_by`), a missing or unreadable pinned AI profile.
