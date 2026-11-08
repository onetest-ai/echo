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

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`small`) | the lead (`serve.sh up`) | all |
| Settings → Recording open | stage 4 (step 1) | 4–9 (one visit; each switch is left as its stage set it: no later stage reads them) |
| Settings → About me open | stage 10 (step 1) | 10–11 (the name and profile are left as set: no later stage reads them) |
| Settings → Templates open, tab "Minutes 4" | stage 12 (step 1) | 12–23 |
| Status sync's editor open | stage 14 (step 1) | 14–21 (each edit undone by its stage's one-click "Reset to built-in", which the next stage's first check needs) |
| Status sync Edited with `Two lines on the week for Acme.` | stage 15 (step 2) | 15–16 (stage 16's Reset is stage 15's teardown) |
| The template "Client kickoff" (Yours) | stage 22 (steps 1–3) | 22–23 (stage 23's Delete is stage 22's teardown) |
| Tab "Meeting types 6" open | stage 24 (step 1) | 24–27 |
| The meeting type Client Edited | stage 26 (step 2) | 26–27 (stage 27's Reset is stage 26's teardown) |
| Calls view with the Vocabulary panel | stage 29 (step 1) | 29–32 |
| The Vocabulary panel open | stage 30 (step 1) | 30–32 |
| Two learned terms (Berlin pilot, Okta SSO) and the panel showing them | stage 33 (steps 1–2) | 33–34 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `settings/TC-001_every-section-opens.md` | — | — |
| 2 | `settings/TC-002_search-settings-filters-names.md` | — | On stage 1's view (Settings, 12 sections in the left list): skip step 1's navigation, check its expected result on one snapshot |
| 3 | `settings/TC-003_appearance-dark-light-system.md` | — | — |
| 4 | `settings/TC-004_recording-calendar-meetings-manual.md` | — | Skip its teardown (Calendar meetings stays Manual: no later stage reads it) |
| 5 | `settings/TC-005_recording-other-calls-manual.md` | — | On stage 4's view (`#settings/recording`): skip step 1's navigation, read the row. Skip its teardown |
| 6 | `settings/TC-006_recording-add-remove-language.md` | — | On stage 5's view: skip step 1's navigation, read the chips |
| 7 | `settings/TC-007_recording-new-recordings-detect.md` | — | On stage 6's view: skip step 1's navigation, read the row. Keep its teardown (the choice is in the browser, not the data) |
| 8 | `settings/TC-008_recording-follow-teams-mute-off.md` | — | On stage 7's view: skip step 1's navigation, read the switch. Skip its teardown |
| 9 | `settings/TC-009_recording-names-from-teams-window-off.md` | — | On stage 8's view: skip step 1's navigation, read the switch. Skip its teardown |
| 10 | `settings/TC-021_about-me-name-saved.md` | — | Skip its teardown (the name stays `Sam Rivera Jr`: no later stage reads it) |
| 11 | `settings/TC-022_about-me-profile-edit-save.md` | — | On stage 10's view (`#settings/me`): skip step 1's navigation, read the card. Skip its teardown (the profile text stays: no later stage reads `me.md`) |
| 12 | `templates/TC-001_templates-tabs-and-counts.md` | — | — |
| 13 | `templates/TC-002_minutes-templates-list-kinds-and-use.md` | — | On stage 12's view (tab "Minutes 4" selected): skip step 1's navigation, read the list |
| 14 | `templates/TC-003_open-built-in-template-editor.md` | — | On stage 13's view: in step 1 skip the navigation, click the row "Status sync" |
| 15 | `templates/TC-004_edit-built-in-template-makes-it-edited.md` | — | On stage 14's view: in step 1 skip the navigation (click the row, run the `ls`). Skip its teardown: keep Status sync Edited for stage 16 |
| 16 | `templates/TC-005_reset-to-built-in-brings-echos-back.md` | 15 | On stage 15's view (Status sync's editor, tag "Edited", Summary holding `Two lines on the week for Acme.`): skip step 1, check its expected result on one snapshot; go on with step 2. The trash count in step 4 is 1 |
| 17 | `templates/TC-006_title-words-added-and-removed.md` | — | On stage 16's view (Status sync's editor open, Built in): skip step 1's navigation and click, check its expected result on one snapshot |
| 18 | `templates/TC-007_section-moved-down-changes-order.md` | — | On stage 17's view (after its Reset): skip step 1's navigation and click, check its expected result on one snapshot |
| 19 | `templates/TC-008_section-added-and-named.md` | — | On stage 18's view (after its Reset): skip step 1's navigation and click, check its expected result on one snapshot |
| 20 | `templates/TC-009_section-removed.md` | — | On stage 19's view (after its Reset): skip step 1's navigation and click, check its expected result on one snapshot |
| 21 | `templates/TC-010_header-value-inserted-from-the-info-card.md` | — | On stage 20's view (after its Reset): skip step 1's navigation and click, check its expected result on one snapshot. Keep its teardown (stage 28 reads `status-sync` as built in) |
| 22 | `templates/TC-011_new-template-takes-its-id-from-its-name.md` | — | On stage 21's view: in step 1 skip the navigation, click "New template". Skip its teardown: keep "Client kickoff" for stage 23 |
| 23 | `templates/TC-012_delete-own-template-moves-it-to-the-trash.md` | 22 | On stage 22's view (the row "Client kickoff", tag "Yours", its editor open): in step 1 run only the `ls … \| grep -c` (N) and check the row on one snapshot; skip the navigation, "New template", the name and "Create" |
| 24 | `templates/TC-013_meeting-types-list-echos-six.md` | — | On stage 23's view: in step 1 skip the navigation, click the tab "Meeting types 6" |
| 25 | `templates/TC-014_new-meeting-type-is-yours.md` | — | On stage 24's view (tab "Meeting types 6"): in step 1 skip the navigation and the tab, click "New type" |
| 26 | `templates/TC-015_edit-built-in-meeting-type-makes-it-edited.md` | — | On stage 25's view: in step 1 skip the navigation and the tab, click the row "Client". Skip its teardown: keep Client Edited for stage 27 |
| 27 | `templates/TC-016_reset-meeting-type-brings-echos-back.md` | 26 | On stage 26's view (Client's editor, tag "Edited", "How to tell it" holding `A client or prospect takes part: status, workshop, demo.`): skip step 1, check its expected result on one snapshot; go on with step 2 |
| 28 | `templates/TC-017_cli-templates-lists-what-the-window-shows.md` | — | — |
| 29 | `vocabulary/TC-001_vocabulary-opens-from-the-book-button.md` | — | — |
| 30 | `vocabulary/TC-003_add-an-every-call-term.md` | — | On stage 29's view (Calls, panel closed): in step 1 skip the reload, click the book button "Vocabulary". Its teardown as written (both chips removed: stage 31 starts from "None yet.") |
| 31 | `vocabulary/TC-004_remove-an-every-call-term.md` | 30 | On stage 30's view (the panel open): in step 1 skip the reload and the book button; add "Kubernetes" |
| 32 | `vocabulary/TC-005_the-same-term-twice-is-one-chip.md` | — | On stage 31's view (the panel open): in step 1 skip the reload and the book button, type `Okta SSO` and press Enter |
| 33 | `vocabulary/TC-006_learned-terms-show-their-meetings-and-people.md` | — | Skip its teardown: keep both learned terms, and the panel open on them, for stage 34 |
| 34 | `vocabulary/TC-007_always-moves-a-learned-term-to-every-call.md` | 33 | On stage 33's view (the panel open, "Learned from calls" `2`): skip step 1 (stage 33's step 1 ran the same command and checked its output); in step 2 skip the reload and the book button, click "Always" in the row "Okta SSO" |
| 35 | `vocabulary/TC-008_remove-forgets-a-learned-term.md` | — | — |
| 36 | `vocabulary/TC-009_learned-filter-appears-past-six-terms.md` | — | — |

## Notes
- The stages share state instead of undoing it: a switch, a language choice, the name and the profile stay as their stage set them when no later stage reads them (stages 4, 5, 8–11), and an edit stays for the next stage that would make it again (stages 15→16, 22→23, 26→27, 30→31, 33→34). Teardowns that a later stage's first check needs are kept: the template Resets (stages 17–21, so each starts from Echo's Status sync and stage 28 reads it built in), the vocabulary removals before a CLI step, and the two choices kept in the browser (stages 3 and 7).
- Each section is opened once for the stages on it: Recording (4–9), About me (10–11), Templates (12–27), the Vocabulary panel (29–32).
- The sections stage and the About me stages never assert the General and Health values (they read the real machine); no screenshot of those two sections is kept.
- Stage 30's step 5 runs `echoctl vocab list` with two terms in Every call: they stay there (#106).
