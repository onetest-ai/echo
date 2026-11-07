---
id: S-11
title: Set Echo up your way: appearance, recording, languages, About me, a template, a meeting type, the vocabulary
run: agent
data: small
cases:
  - tests/cases/settings/TC-001_every-section-opens.md
  - tests/cases/settings/TC-002_search-settings-filters-names.md
  - tests/cases/settings/TC-003_appearance-dark-light-system.md
  - tests/cases/settings/TC-004_recording-calendar-meetings-manual.md
  - tests/cases/settings/TC-005_recording-other-calls-manual.md
  - tests/cases/settings/TC-006_recording-add-remove-language.md
  - tests/cases/settings/TC-007_recording-new-recordings-detect.md
  - tests/cases/settings/TC-008_recording-follow-teams-mute-off.md
  - tests/cases/settings/TC-009_recording-names-from-teams-window-off.md
  - tests/cases/settings/TC-021_about-me-name-saved.md
  - tests/cases/settings/TC-022_about-me-profile-edit-save.md
  - tests/cases/templates/TC-001_templates-tabs-and-counts.md
  - tests/cases/templates/TC-002_minutes-templates-list-kinds-and-use.md
  - tests/cases/templates/TC-003_open-built-in-template-editor.md
  - tests/cases/templates/TC-004_edit-built-in-template-makes-it-edited.md
  - tests/cases/templates/TC-005_reset-to-built-in-brings-echos-back.md
  - tests/cases/templates/TC-006_title-words-added-and-removed.md
  - tests/cases/templates/TC-007_section-moved-down-changes-order.md
  - tests/cases/templates/TC-008_section-added-and-named.md
  - tests/cases/templates/TC-009_section-removed.md
  - tests/cases/templates/TC-010_header-value-inserted-from-the-info-card.md
  - tests/cases/templates/TC-011_new-template-takes-its-id-from-its-name.md
  - tests/cases/templates/TC-012_delete-own-template-moves-it-to-the-trash.md
  - tests/cases/templates/TC-013_meeting-types-list-echos-six.md
  - tests/cases/templates/TC-014_new-meeting-type-is-yours.md
  - tests/cases/templates/TC-015_edit-built-in-meeting-type-makes-it-edited.md
  - tests/cases/templates/TC-016_reset-meeting-type-brings-echos-back.md
  - tests/cases/templates/TC-017_cli-templates-lists-what-the-window-shows.md
  - tests/cases/vocabulary/TC-001_vocabulary-opens-from-the-book-button.md
  - tests/cases/vocabulary/TC-003_add-an-every-call-term.md
  - tests/cases/vocabulary/TC-004_remove-an-every-call-term.md
  - tests/cases/vocabulary/TC-005_the-same-term-twice-is-one-chip.md
  - tests/cases/vocabulary/TC-006_learned-terms-show-their-meetings-and-people.md
  - tests/cases/vocabulary/TC-007_always-moves-a-learned-term-to-every-call.md
  - tests/cases/vocabulary/TC-008_remove-forgets-a-learned-term.md
  - tests/cases/vocabulary/TC-009_learned-filter-appears-past-six-terms.md
---

# S-11: Set Echo up your way: appearance, recording, languages, About me, a template, a meeting type, the vocabulary

**Story:** Sam opens Settings on a fresh Echo and makes it his: checks every section opens, picks the appearance, chooses when to record and in which languages, tells Echo who he is, adapts a minutes template and adds a template and a meeting type of his own, then teaches the transcription a few names.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `settings/TC-001_every-section-opens.md` | — | — |
| 2 | `settings/TC-002_search-settings-filters-names.md` | — | — |
| 3 | `settings/TC-003_appearance-dark-light-system.md` | — | — |
| 4 | `settings/TC-004_recording-calendar-meetings-manual.md` | — | — |
| 5 | `settings/TC-005_recording-other-calls-manual.md` | — | — |
| 6 | `settings/TC-006_recording-add-remove-language.md` | — | — |
| 7 | `settings/TC-007_recording-new-recordings-detect.md` | — | — |
| 8 | `settings/TC-008_recording-follow-teams-mute-off.md` | — | — |
| 9 | `settings/TC-009_recording-names-from-teams-window-off.md` | — | — |
| 10 | `settings/TC-021_about-me-name-saved.md` | — | — |
| 11 | `settings/TC-022_about-me-profile-edit-save.md` | — | — |
| 12 | `templates/TC-001_templates-tabs-and-counts.md` | — | — |
| 13 | `templates/TC-002_minutes-templates-list-kinds-and-use.md` | — | — |
| 14 | `templates/TC-003_open-built-in-template-editor.md` | — | — |
| 15 | `templates/TC-004_edit-built-in-template-makes-it-edited.md` | — | — |
| 16 | `templates/TC-005_reset-to-built-in-brings-echos-back.md` | — | — |
| 17 | `templates/TC-006_title-words-added-and-removed.md` | — | — |
| 18 | `templates/TC-007_section-moved-down-changes-order.md` | — | — |
| 19 | `templates/TC-008_section-added-and-named.md` | — | — |
| 20 | `templates/TC-009_section-removed.md` | — | — |
| 21 | `templates/TC-010_header-value-inserted-from-the-info-card.md` | — | — |
| 22 | `templates/TC-011_new-template-takes-its-id-from-its-name.md` | — | — |
| 23 | `templates/TC-012_delete-own-template-moves-it-to-the-trash.md` | — | Click the tab "Minutes 4" |
| 24 | `templates/TC-013_meeting-types-list-echos-six.md` | — | — |
| 25 | `templates/TC-014_new-meeting-type-is-yours.md` | — | — |
| 26 | `templates/TC-015_edit-built-in-meeting-type-makes-it-edited.md` | — | — |
| 27 | `templates/TC-016_reset-meeting-type-brings-echos-back.md` | — | — |
| 28 | `templates/TC-017_cli-templates-lists-what-the-window-shows.md` | — | — |
| 29 | `vocabulary/TC-001_vocabulary-opens-from-the-book-button.md` | — | — |
| 30 | `vocabulary/TC-003_add-an-every-call-term.md` | — | — |
| 31 | `vocabulary/TC-004_remove-an-every-call-term.md` | — | — |
| 32 | `vocabulary/TC-005_the-same-term-twice-is-one-chip.md` | — | — |
| 33 | `vocabulary/TC-006_learned-terms-show-their-meetings-and-people.md` | — | — |
| 34 | `vocabulary/TC-007_always-moves-a-learned-term-to-every-call.md` | — | — |
| 35 | `vocabulary/TC-008_remove-forgets-a-learned-term.md` | — | — |
| 36 | `vocabulary/TC-009_learned-filter-appears-past-six-terms.md` | — | — |

## Notes
- Every change a stage makes is undone in its teardown, so a stage never starts from another's leftovers; the stages are in the order a person would do them.
- The sections stage and the About me stages never assert the General and Health values (they read the real machine); no screenshot of those two sections is kept.
- The vocabulary stages use the CLI after a term was removed from Every call: a term left in Every call would be moved to the learned list by the first `echoctl vocab` command (see the report).
- The delete-template stage creates its own template first.
