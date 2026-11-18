---
id: S-40
title: "A plugin's own routine: named on its sheet, off until switched on, tuned as Echo's jobs, run as the folder session, recorded in Echo's words, and gone with its plugin"
run: agent
data: standard
cases:
  - tests/cases/plugins/TC-110_a-plugins-routine-is-named-on-its-sheet-and-an-update-that-changes-its-prompt-asks-again.md
  - tests/cases/plugins/TC-111_a-plugins-routine-is-listed-off-until-the-user-switches-it-on-and-tuned-as-echos-jobs.md
  - tests/cases/plugins/TC-112_a-plugins-routine-runs-as-the-folder-session-and-its-run-is-recorded-in-echos-words.md
  - tests/cases/plugins/TC-113_a-routine-of-a-plugin-thats-off-or-gone-doesnt-run-and-a-run-never-starts-or-switches-one.md
  - tests/cases/plugins/TC-227_a-plugins-routines-are-switched-on-from-its-install-sheet.md
  - tests/cases/plugins/TC-228_plugin-install-and-update-switch-routines-on-with-the-routines-flag.md
  - tests/cases/plugins/TC-229_an-updates-sheet-switches-only-the-routines-it-adds-or-changes.md
  - tests/cases/plugins/TC-230_a-plugins-setup-offers-its-routines-on-echos-sheet.md
---

# S-40: A plugin's own routine, from its sheet to its runs

**Story:** Sam installs a plugin that ships a routine of its own ("Mail → notes"). Its sheet names the routine and what its run may do; an update that changes its prompt asks again. The routine is listed with Echo's jobs and stays off until Sam switches it on in Settings → Plugins; he tunes it as he tunes Echo's jobs, and a plugin that tunes jobs can't switch it. Run now, it goes as the folder session on his default profile; its run is recorded in Echo's words and its answer is his to read. Switched off with its plugin, or removed, it doesn't run. A plugin with two routines lets him switch one on right on its install sheet (or with `--routines`), an update's sheet asks only about the routine it adds, and the plugin's own setup opens Echo's sheet of its routines on his click.

**Data:** `standard` with the fake Claude Code: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`.

**Length:** about 10 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build with the fake Claude Code | the lead (`serve.sh up … --ai fake`) | 1–8 |
| The test marketplace (`tests/fixtures/plugins`, its copy `$C` in stages 1, 6 and 7) | each stage's preconditions | 1–8 (each stage's teardown removes it) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-110_a-plugins-routine-is-named-on-its-sheet-and-an-update-that-changes-its-prompt-asks-again.md` | — | Its marketplace is its own copy of the test marketplace (`$C`): run its precondition lines as written |
| 2 | `plugins/TC-111_a-plugins-routine-is-listed-off-until-the-user-switches-it-on-and-tuned-as-echos-jobs.md` | — | Run its precondition lines as written (stage 1's teardown removed `test-plugins`); load `{{base_url}}/` first |
| 3 | `plugins/TC-112_a-plugins-routine-runs-as-the-folder-session-and-its-run-is-recorded-in-echos-words.md` | — | Run its precondition lines as written: the fake `claude` answers the routine from `$D/fake-answers.tsv` |
| 4 | `plugins/TC-113_a-routine-of-a-plugin-thats-off-or-gone-doesnt-run-and-a-run-never-starts-or-switches-one.md` | — | Run its precondition lines as written |
| 5 | `plugins/TC-227_a-plugins-routines-are-switched-on-from-its-install-sheet.md` | — | Run its precondition lines as written; load `{{base_url}}/` first |
| 6 | `plugins/TC-228_plugin-install-and-update-switch-routines-on-with-the-routines-flag.md` | — | Run its precondition lines as written |
| 7 | `plugins/TC-229_an-updates-sheet-switches-only-the-routines-it-adds-or-changes.md` | — | Run its precondition lines as written; load `{{base_url}}/` first |
| 8 | `plugins/TC-230_a-plugins-setup-offers-its-routines-on-echos-sheet.md` | — | Run its precondition lines as written; load `{{base_url}}/` first |

## Notes
- Each stage installs Routine demo (stages 5–8: Routines demo) from the test marketplace and removes both in its teardown: the stages don't share a plugin, only the data.
- The app's schedule (a routine that is on runs every N minutes) never runs on test data; `echoctl jobs run` runs one now (stage 3). The schedule itself is the Rust test `plugins::routines::a_routine_is_due_every_n_minutes_from_its_last_run_off_and_at_night_never`.
