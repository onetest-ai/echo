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
---

# S-29: Notes, transcripts, settings and Today off the happy path

**Story:** Sam goes back over a busy day. He opens a meeting made from its Teams transcript and two calls whose transcripts aren't there, ticks and edits his notes on the Acme sync, comments on what was said and on a line of the minutes, starts a note and jumps to another call before adding it, then presses Write minutes with a note still in the box. A notes file that can't be read, and one another editor changed while he typed, tell him what happened. A broken settings file says so instead of the settings. On Today a routine's morning brief waits until he says Got it, and ⌘K acts on the day's preps and tasks.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run, built right before it and before 21:00 local (stage 10 needs meetings still to come).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `window/TC-022_a-call-made-from-a-teams-transcript-shows-its-speakers.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `window/TC-020_a-transcript-tab-without-a-transcript-says-why.md` | — | — |
| 3 | `window/TC-016_a-to-do-s-box-ticks-it-done-and-back.md` | — | Open My notes (`[aria-label="My notes"]`) on a call if it isn't open |
| 4 | `window/TC-017_a-note-edited-in-place-saves-cancels-and-is-deleted.md` | — | — |
| 5 | `window/TC-021_comment-on-words-or-a-line-adds-a-note-at-that-moment.md` | — | — |
| 6 | `window/TC-018_a-note-typed-but-not-added-is-kept-when-another-call-opens-or-write-minutes.md` | — | — |
| 7 | `window/TC-019_unreadable-notes-say-so-and-an-outside-change-while-typing-is-kept-beside.md` | — | — |
| 8 | `settings/TC-025_unreadable-config-says-so-instead-of-the-sections.md` | — | — |
| 9 | `today/TC-004_a-card-sent-to-today-shows-until-got-it.md` | — | — |
| 10 | `today/TC-005_actions-on-a-busy-today-act-on-its-items.md` | — | — |

## Notes
- Stages 3–7 change the notes of their calls (each on its own calls; stage 5 adds a note to today's Acme weekly sync after stage 4 edited it, and expects only its own line). Stage 8 breaks `config.json` and restores it: its teardown must run.
- Stage 5 is expected to FAIL (steps 6, 8–9) until the suspected bugs it checks are fixed: a right-clicked turn's note saved a second earlier than the menu says, and a minutes line's quote holding its time chip and the notes shown under it.
- Stage 7 is expected to FAIL (steps 5–7): a note typed in the box while the file is changed outside is added to the outside version, with no copy and no notice.
- Not covered here: the window closing with a note typed (native window), "Listening…" while a call records (the test call), the Teams transcript import itself (Microsoft 365), the quiet day (`today/TC-006`, in S-10 on the `small` set).
