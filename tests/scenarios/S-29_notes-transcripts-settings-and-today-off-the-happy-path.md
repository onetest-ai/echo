---
id: S-29
title: "Notes, transcripts, settings and Today off the happy path: imports, empty transcripts, to-dos, edits, comments, notes not lost, broken files, cards and the day's actions"
run: agent
data: standard
cases:
  - tests/cases/window/TC-022_a-call-made-from-a-teams-transcript-shows-its-speakers.md
  - tests/cases/window/TC-020_a-transcript-tab-without-a-transcript-says-why.md
  - tests/cases/window/TC-016_a-to-do-s-box-ticks-it-done-and-back.md
  - tests/cases/window/TC-017_a-note-edited-in-place-saves-cancels-and-is-deleted.md
  - tests/cases/window/TC-021_comment-on-words-or-a-line-adds-a-note-at-that-moment.md
  - tests/cases/window/TC-018_a-note-typed-but-not-added-is-kept-when-another-call-opens-or-write-minutes.md
  - tests/cases/window/TC-019_unreadable-notes-say-so-and-an-outside-change-while-typing-is-kept-beside.md
  - tests/cases/settings/TC-025_unreadable-config-says-so-instead-of-the-sections.md
  - tests/cases/today/TC-004_a-card-sent-to-today-shows-until-got-it.md
  - tests/cases/today/TC-005_actions-on-a-busy-today-act-on-its-items.md
  - tests/cases/today/TC-012_an-agenda-from-another-day-isnt-listed-and-the-day-says-so.md
---

# S-29: Notes, transcripts, settings and Today off the happy path

**Story:** Sam goes back over a busy day. He opens a meeting made from its Teams transcript and two calls whose transcripts aren't there, ticks and edits his notes on the Acme sync, comments on what was said and on a line of the minutes, starts a note and jumps to another call before adding it, then presses Write minutes with a note still in the box. A notes file that can't be read, and one another editor changed while he typed, tell him what happened. A broken settings file says so instead of the settings. On Today a routine's morning brief waits until he says Got it, and ⌘K acts on the day's preps and tasks.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run, built right before it and before 21:00 local (stage 10 needs meetings still to come).

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`standard`) | the lead (`serve.sh up … --set standard`), right before the run and before 21:00 | all |
| The call folders' names (dated by the build) | Prelude, step P2 (one `ls`) | 1, 3–7 (the shell steps and the folder names in TC-018's and TC-019's Test Data) |
| The Calls list on screen | stage 1 (step 1) | 1–3, 5, 6 (each clicks its row without navigating again) |
| My notes open in the right panel, on today's Acme weekly sync | stage 3 (step 1) | 3–7 (the panel stays open as calls change; stage 4 reopens it after its reload, step 6) |
| Today on screen, the card dismissed | stage 9 (step 7) | 10 |

## Prelude

Done once, before stage 1, timed as the stage `prelude`.

| # | Action | Expected Result |
|---|---|---|
| P1 | Load `{{base_url}}/` (the cookie) in the open browser | The window loads |
| P2 | `. <dir>/env; ls "$ECHOCTL_HOME/calls" \| grep -E -- '-(globex-steering\|acme-weekly-sync\|globex-standup\|umbrella-discovery\|duff-discovery\|coffee-chat\|initech-planning)$'` | One folder each, three for `acme-weekly-sync` (two older, today's last). Note them: their dates follow the build day, so a case's Test Data that names a dated folder (TC-018, TC-019) means the folder of the same slug here — the older Acme weekly sync whose `my-notes.md` is a folder (`ls -ld …/my-notes.md` → `d`) for TC-019 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `window/TC-022_a-call-made-from-a-teams-transcript-shows-its-speakers.md` | — | — |
| 2 | `window/TC-020_a-transcript-tab-without-a-transcript-says-why.md` | — | On stage 1's view (the Calls list, Globex steering open): in step 1 skip the navigation, click the row |
| 3 | `window/TC-016_a-to-do-s-box-ticks-it-done-and-back.md` | — | On stage 2's view (the Calls list): in step 1 skip the navigation, click the row; step 1 opens My notes, which stays open for stages 3–7 |
| 4 | `window/TC-017_a-note-edited-in-place-saves-cancels-and-is-deleted.md` | — | On stage 3's view (today's Acme weekly sync, My notes open: its Preconditions) |
| 5 | `window/TC-021_comment-on-words-or-a-line-adds-a-note-at-that-moment.md` | — | On stage 4's view (the Calls list, My notes open): in step 1 skip the navigation, click the row and the tab |
| 6 | `window/TC-018_a-note-typed-but-not-added-is-kept-when-another-call-opens-or-write-minutes.md` | — | On stage 5's view (the Calls list, My notes open): in step 1 skip the navigation. Its folders are the prelude's of the same slug (Umbrella discovery, Duff discovery, Coffee chat) |
| 7 | `window/TC-019_unreadable-notes-say-so-and-an-outside-change-while-typing-is-kept-beside.md` | — | My notes is open since stage 3. Step 1's folder is the prelude's older Acme weekly sync whose `my-notes.md` is a folder |
| 8 | `settings/TC-025_unreadable-config-says-so-instead-of-the-sections.md` | — | — |
| 9 | `today/TC-004_a-card-sent-to-today-shows-until-got-it.md` | — | Steps 1–2 in one shell call (each command preceded by `echo "== step N"`, followed by `echo "exit $?"`; each step checked on its own part). Skip nothing else: Today stays on screen for stage 10 |
| 10 | `today/TC-005_actions-on-a-busy-today-act-on-its-items.md` | — | On stage 9's view (Your day, no card): in step 1 skip the navigation, check "Your day" on one snapshot, press `ControlOrMeta+k` |
| 11 | `today/TC-012_an-agenda-from-another-day-isnt-listed-and-the-day-says-so.md` | — | After stage 10 (it changes the agenda; its step 5 puts it back). Steps 1 and 5's shell parts in one shell call each |

## Notes
- Stages 3–7 change the notes of their calls (each on its own calls; stage 5 adds a note to today's Acme weekly sync after stage 4 edited it, and expects only its own line). Stage 8 breaks `config.json` and restores it: its teardown must run.
- My notes is opened once (stage 3) and stays open while stages 3–7 change calls; a stage that finds it closed opens it (`[aria-label="My notes"]`) before its first step and says so in its notes.
- The call folders are dated by the build: the prelude lists them once; TC-018's and TC-019's Test Data name folders with the dates of the day they were written (reported: those names are stale on any other day), so the runner uses the prelude's folder of the same slug.
- Stage 5 is expected to FAIL (steps 6, 8–9) until the suspected bugs it checks are fixed: a right-clicked turn's note saved a second earlier than the menu says, and a minutes line's quote holding its time chip and the notes shown under it.
- Not covered here: the window closing with a note typed (native window), "Listening…" while a call records (the test call), the Teams transcript import itself (Microsoft 365), the quiet day (`today/TC-006`, in S-10 on the `small` set).
