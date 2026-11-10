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
  - tests/cases/settings/TC-027_ai-account-chip-shows-and-picks-the-default.md
  - tests/cases/settings/TC-016_ai-profiles-add-local-shows-picker.md
  - tests/cases/settings/TC-017_ai-make-default-profile.md
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
  - tests/cases/plugins/TC-010_add-a-marketplace-from-a-folder.md
  - tests/cases/plugins/TC-002_discover-chips-search-and-cards.md
  - tests/cases/plugins/TC-011_hide-and-show-a-marketplaces-plugins.md
  - tests/cases/plugins/TC-003_install-sheet-shows-what-the-plugin-uses-and-cancels.md
  - tests/cases/plugins/TC-004_install-through-the-sheet-adds-the-plugin.md
  - tests/cases/plugins/TC-005_plugin-own-settings-are-saved.md
  - tests/cases/plugins/TC-006_plugin-switch-turns-it-off-and-on.md
  - tests/cases/plugins/TC-007_remove-asks-before-it-removes.md
  - tests/cases/plugins/TC-008_remove-keep-data-leaves-its-settings.md
  - tests/cases/plugins/TC-012_remove-a-marketplace-from-the-menu.md
auto: [tests/auto/specs/S-12.spec.mjs]
---

# S-12: Skills and AI: skills on and off, AI profiles and the default, plugins and marketplaces

**Story:** Sam looks at what Claude can do for him: the follow-up mail skill (on, off, opened, customized and reset), the AI profiles (one, then a local model added and made the default, in Settings and from the account chip in the top bar), the Calendar and Sends-to-you states, and then Plugins: Discover, a marketplace of his own, an install through the sheet, its settings, the switch and the removal.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`small`) | the lead (`serve.sh up`) | all |
| Templates → tab "Skills 1" open | stage 1 (step 1) | 1–6 |
| The customized copy of "Follow up mail" (and the SKILL.md window Echo opens, closed once) | stage 4 (step 2) | 4–5 (stage 5's Reset is stage 4's teardown) |
| The AI profile Local Llama (`local-llama`) | stage 8 (step 4) | 8–10 (removed by stage 10's teardown) |
| Settings → AI & minutes open | stage 9 (step 2) | 9–13 |
| Settings → Transcription open | stage 14 (step 1) | 14–16 |
| The made-up marketplace folder `$M` | the prelude | 22–31 |
| Settings → Plugins open | stage 19 (step 1) | 19–31 |
| The marketplace `acme-local`, added in the window | stage 22 (step 1) | 22–31 (removed by stage 31) |
| The plugin Acme notes, installed through the sheet | stage 26 (step 2) | 26–30 (removed by stage 30) |

## Prelude

1. Load `{{base_url}}/`.
2. Write the made-up marketplace once, in Bash: `. <dir>/env; M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`, then the two `echo … >` lines of the Preconditions of `plugins/TC-010_add-a-marketplace-from-a-folder.md` (`echo-marketplace.json` and `plugins/acme-notes/echo-plugin.json`). The plugin stages use this folder; none writes it again.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `skills/TC-001_skills-list-built-in-follow-up-mail.md` | — | — |
| 2 | `skills/TC-002_open-skill-shows-skill-md.md` | — | On stage 1's view (tab "Skills 1"): in step 1 skip the navigation and the tab, click the row "Follow up mail" |
| 3 | `skills/TC-003_skill-off-is-kept-out-of-claude.md` | — | On stage 2's view: in step 1 skip the navigation and the tab, run the `cat` |
| 4 | `skills/TC-004_customize-copies-the-built-in-skill.md` | — | On stage 3's view: in step 1 skip the navigation and the tab, click the row "Follow up mail". Skip its teardown: keep the customized copy for stage 5 |
| 5 | `skills/TC-005_reset-takes-the-customized-copy-away.md` | 4 | On stage 4's view (the editor, tag "Customized"): skip step 1, check its expected result on one snapshot; go on with step 2 (N is 0) |
| 6 | `skills/TC-006_new-skill-name-must-be-latin.md` | — | On stage 5's view: in step 1 skip the navigation and the tab, click "New skill" |
| 7 | `settings/TC-015_ai-profiles-one-built-in.md` | — | — |
| 8 | `settings/TC-027_ai-account-chip-shows-and-picks-the-default.md` | — | In its teardown run only `echoctl ai default system`: keep the profile Local Llama for stages 9–10 |
| 9 | `settings/TC-016_ai-profiles-add-local-shows-picker.md` | 8 | Local Llama is there from stage 8 (step 4): skip step 1 (stage 8 ran the same command and checked its output). Skip its teardown: keep Local Llama for stage 10 |
| 10 | `settings/TC-017_ai-make-default-profile.md` | 8 | On stage 9's view (`#settings/ai`, Local Llama with "Make default"): skip step 1; in step 2 skip the navigation, click "Make default" on the row "Local Llama" |
| 11 | `settings/TC-018_ai-live-notes-switch-and-status-bar.md` | — | On stage 10's view (`#settings/ai`; the window reads the AI profiles again every 2 s, so stage 10's teardown shows by itself): skip step 1's navigation, read the switch and the status bar once the card "Profiles" shows one row. Skip its teardown (live notes stay on: no later stage reads them) |
| 12 | `settings/TC-013_ai-write-minutes-automatically-off.md` | — | On stage 11's view: skip step 1's navigation, read the switch. Skip its teardown (no later stage reads it) |
| 13 | `settings/TC-014_ai-claude-in-window-no-prompts-warning.md` | — | On stage 12's view: skip step 1's navigation, find the card |
| 14 | `settings/TC-010_transcription-models-listed-not-downloaded.md` | — | — |
| 15 | `settings/TC-011_transcription-keep-audio-files-on.md` | — | On stage 14's view (`#settings/transcription`): skip step 1's navigation, read the switch. Skip its teardown (no later stage reads it) |
| 16 | `settings/TC-012_transcription-echo-cancellation-off.md` | — | On stage 15's view: skip step 1's navigation, read the switch. Skip its teardown (no later stage reads it) |
| 17 | `settings/TC-019_calendar-and-chat-not-connected.md` | — | — |
| 18 | `settings/TC-020_sends-to-you-needs-sign-in.md` | — | — |
| 19 | `plugins/TC-001_plugins-none-installed-opens-on-discover.md` | — | — |
| 20 | `plugins/TC-009_marketplaces-tab-shows-the-built-in-one.md` | — | On stage 19's view (`#settings/plugins`): in step 1 skip the navigation, click the tab "Marketplaces" |
| 21 | `plugins/TC-013_cli-plugin-list-matches-the-window.md` | — | — |
| 22 | `plugins/TC-010_add-a-marketplace-from-a-folder.md` | — | The folder `$M` is the prelude's: of the precondition lines run only `M="$(dirname "$ECHOCTL_HOME")/market"` (in each Bash call that uses `$M`). On stage 20's view (tab "Marketplaces"): in step 1 skip the navigation and the tab, fill "Marketplace" and "Name" and click "Add". Skip its teardown: keep `acme-local` for stages 23–31 |
| 23 | `plugins/TC-002_discover-chips-search-and-cards.md` | 22 | `acme-local` is there from stage 22: skip the precondition lines and step 1. On stage 22's view: in step 2 skip the navigation, click the tab "Discover". Skip its teardown |
| 24 | `plugins/TC-011_hide-and-show-a-marketplaces-plugins.md` | 22 | `acme-local` is there from stage 22: skip the precondition lines and step 1. On stage 23's view: in step 2 skip the navigation (click the tab "Marketplaces", "⋯", "Hide…"). Skip its teardown |
| 25 | `plugins/TC-003_install-sheet-shows-what-the-plugin-uses-and-cancels.md` | 22 | `acme-local` is there from stage 22: skip the precondition lines and step 1. On stage 24's view (Discover, `acme` in the search, the card "Acme notes"): in step 2 skip the navigation, the tab and the typing, click "Install" on the card. Skip its teardown |
| 26 | `plugins/TC-004_install-through-the-sheet-adds-the-plugin.md` | 22 | `acme-local` is there from stage 22: skip the precondition lines and step 1. On stage 25's view (Discover, `acme` in the search): in step 2 skip the navigation, the tab and the typing, click "Install" on the card, then "Install" in the dialog. Skip its teardown: keep Acme notes installed for stages 27–30 |
| 27 | `plugins/TC-005_plugin-own-settings-are-saved.md` | 26 | Acme notes is installed by stage 26: skip the precondition lines. On stage 26's view (tab "Installed", the detail of Acme notes): in step 2 skip the navigation, read "Its settings". Skip its teardown |
| 28 | `plugins/TC-006_plugin-switch-turns-it-off-and-on.md` | 26 | Acme notes is installed by stage 26: skip the precondition lines. On stage 27's view: skip step 1's navigation, check its expected result on one snapshot. Skip its teardown |
| 29 | `plugins/TC-007_remove-asks-before-it-removes.md` | 26 | Acme notes is installed by stage 26: skip the precondition lines. On stage 28's view: in step 1 skip the navigation, click "Remove…". Skip its teardown |
| 30 | `plugins/TC-008_remove-keep-data-leaves-its-settings.md` | 26 | Acme notes is installed by stage 26: skip the precondition lines. On stage 29's view: in step 2 skip the navigation (click "Remove…", then "Remove, keep data"). In its teardown run only the `rm -r`: keep `acme-local` for stage 31 |
| 31 | `plugins/TC-012_remove-a-marketplace-from-the-menu.md` | 22 | `acme-local` is there from stage 22: skip the precondition lines and step 1. On stage 30's view: in step 2 skip the navigation (click the tab "Marketplaces" and "⋯" on "acme-local") |

## Notes
- The plugin stages use the made-up local marketplace the prelude writes in the run's scratch folder; nothing is fetched from the network, and the built-in marketplace's own plugins are never installed. It is added once (stage 22, in the window), installed from once (stage 26, through the sheet), and the plugin's settings, switch and removal follow on that install; stage 31 removes the marketplace.
- The no-plugin stages (19–21: Installed empty, the built-in marketplace alone, the CLI list) come before the marketplace is added.
- The customize and reset stages open SKILL.md in this computer's default app (Echo's `open`) once, in stage 4; close that window.
- The AI stages add one local model profile (stage 8, after the account chip was read with one profile), and the Settings stages reuse it before stage 10 removes it (no model runs on it; `echoctl ai status` only sends `localhost:11434` one read-only `GET /api/ps` and loads nothing). The audio switches ride with the AI stages: they are all "how Echo handles a call" switches. A switch no later stage reads stays as its stage set it (stages 11, 12, 15, 16).
