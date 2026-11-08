---
id: S-12
title: Skills and AI: skills on and off, AI profiles and the default, plugins and marketplaces
run: agent
data: small
cases:
  - tests/cases/skills/TC-001_skills-list-built-in-follow-up-mail.md
  - tests/cases/skills/TC-002_open-skill-shows-skill-md.md
  - tests/cases/skills/TC-003_skill-off-is-kept-out-of-claude.md
  - tests/cases/skills/TC-004_customize-copies-the-built-in-skill.md
  - tests/cases/skills/TC-005_reset-takes-the-customized-copy-away.md
  - tests/cases/skills/TC-006_new-skill-name-must-be-latin.md
  - tests/cases/settings/TC-015_ai-profiles-one-built-in.md
  - tests/cases/settings/TC-016_ai-profiles-add-local-shows-picker.md
  - tests/cases/settings/TC-017_ai-make-default-profile.md
  - tests/cases/settings/TC-027_ai-account-chip-shows-and-picks-the-default.md
  - tests/cases/settings/TC-018_ai-live-notes-switch-and-status-bar.md
  - tests/cases/settings/TC-013_ai-write-minutes-automatically-off.md
  - tests/cases/settings/TC-014_ai-claude-in-window-no-prompts-warning.md
  - tests/cases/settings/TC-010_transcription-models-listed-not-downloaded.md
  - tests/cases/settings/TC-011_transcription-keep-audio-files-on.md
  - tests/cases/settings/TC-012_transcription-echo-cancellation-off.md
  - tests/cases/settings/TC-019_calendar-and-chat-not-connected.md
  - tests/cases/settings/TC-020_sends-to-you-needs-sign-in.md
  - tests/cases/plugins/TC-001_plugins-none-installed-opens-on-discover.md
  - tests/cases/plugins/TC-009_marketplaces-tab-shows-the-built-in-one.md
  - tests/cases/plugins/TC-013_cli-plugin-list-matches-the-window.md
  - tests/cases/plugins/TC-002_discover-chips-search-and-cards.md
  - tests/cases/plugins/TC-003_install-sheet-shows-what-the-plugin-uses-and-cancels.md
  - tests/cases/plugins/TC-004_install-through-the-sheet-adds-the-plugin.md
  - tests/cases/plugins/TC-005_plugin-own-settings-are-saved.md
  - tests/cases/plugins/TC-006_plugin-switch-turns-it-off-and-on.md
  - tests/cases/plugins/TC-007_remove-asks-before-it-removes.md
  - tests/cases/plugins/TC-008_remove-keep-data-leaves-its-settings.md
  - tests/cases/plugins/TC-010_add-a-marketplace-from-a-folder.md
  - tests/cases/plugins/TC-011_hide-and-show-a-marketplaces-plugins.md
  - tests/cases/plugins/TC-012_remove-a-marketplace-from-the-menu.md
---

# S-12: Skills and AI: skills on and off, AI profiles and the default, plugins and marketplaces

**Story:** Sam looks at what Claude can do for him: the follow-up mail skill (on, off, opened, customized and reset), the AI profiles (one, then a local model added and made the default, in Settings and from the account chip in the top bar), the Calendar and Sends-to-you states, and then Plugins: Discover, a marketplace of his own, an install through the sheet, its settings, the switch and the removal.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `skills/TC-001_skills-list-built-in-follow-up-mail.md` | — | — |
| 2 | `skills/TC-002_open-skill-shows-skill-md.md` | — | — |
| 3 | `skills/TC-003_skill-off-is-kept-out-of-claude.md` | — | — |
| 4 | `skills/TC-004_customize-copies-the-built-in-skill.md` | — | — |
| 5 | `skills/TC-005_reset-takes-the-customized-copy-away.md` | — | — |
| 6 | `skills/TC-006_new-skill-name-must-be-latin.md` | — | — |
| 7 | `settings/TC-015_ai-profiles-one-built-in.md` | — | — |
| 8 | `settings/TC-016_ai-profiles-add-local-shows-picker.md` | — | — |
| 9 | `settings/TC-017_ai-make-default-profile.md` | — | — |
| 10 | `settings/TC-027_ai-account-chip-shows-and-picks-the-default.md` | — | — |
| 11 | `settings/TC-018_ai-live-notes-switch-and-status-bar.md` | — | — |
| 12 | `settings/TC-013_ai-write-minutes-automatically-off.md` | — | — |
| 13 | `settings/TC-014_ai-claude-in-window-no-prompts-warning.md` | — | — |
| 14 | `settings/TC-010_transcription-models-listed-not-downloaded.md` | — | — |
| 15 | `settings/TC-011_transcription-keep-audio-files-on.md` | — | — |
| 16 | `settings/TC-012_transcription-echo-cancellation-off.md` | — | — |
| 17 | `settings/TC-019_calendar-and-chat-not-connected.md` | — | — |
| 18 | `settings/TC-020_sends-to-you-needs-sign-in.md` | — | — |
| 19 | `plugins/TC-001_plugins-none-installed-opens-on-discover.md` | — | — |
| 20 | `plugins/TC-009_marketplaces-tab-shows-the-built-in-one.md` | — | — |
| 21 | `plugins/TC-013_cli-plugin-list-matches-the-window.md` | — | — |
| 22 | `plugins/TC-002_discover-chips-search-and-cards.md` | — | — |
| 23 | `plugins/TC-003_install-sheet-shows-what-the-plugin-uses-and-cancels.md` | — | — |
| 24 | `plugins/TC-004_install-through-the-sheet-adds-the-plugin.md` | — | — |
| 25 | `plugins/TC-005_plugin-own-settings-are-saved.md` | — | — |
| 26 | `plugins/TC-006_plugin-switch-turns-it-off-and-on.md` | — | — |
| 27 | `plugins/TC-007_remove-asks-before-it-removes.md` | — | — |
| 28 | `plugins/TC-008_remove-keep-data-leaves-its-settings.md` | — | — |
| 29 | `plugins/TC-010_add-a-marketplace-from-a-folder.md` | — | — |
| 30 | `plugins/TC-011_hide-and-show-a-marketplaces-plugins.md` | — | — |
| 31 | `plugins/TC-012_remove-a-marketplace-from-the-menu.md` | — | — |

## Notes
- The plugin stages use a made-up local marketplace the stages build in the run's scratch folder; nothing is fetched from the network, and the built-in marketplace's own plugins are never installed.
- The customize and reset stages open SKILL.md in this computer's default app (Echo's `open`); close that window.
- The AI stages add, default and remove a local model profile (`localhost:11434` is never contacted). The audio switches ride with the AI stages: they are all "how Echo handles a call" switches.
