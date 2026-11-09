---
id: S-13
title: Find your way around: the activity bar, the top bar, panels, layout, the actions palette, the status bar and shortcuts
run: agent
data: small
cases:
  - tests/cases/shell/TC-001_activity-bar-items-and-order.md
  - tests/cases/shell/TC-002_sidebar-expand-and-fold-is-remembered.md
  - tests/cases/shell/TC-003_sidebar-flips-with-the-keyboard.md
  - tests/cases/shell/TC-004_number-shortcuts-open-the-activities.md
  - tests/cases/shell/TC-011_inbox-button-shows-and-hides-the-right-panel.md
  - tests/cases/shell/TC-012_right-panel-starts-closed-and-keeps-its-last-tab.md
  - tests/cases/shell/TC-013_a-panel-view-moves-to-the-bottom-panel.md
  - tests/cases/settings/TC-023_layout-move-vocabulary-to-bottom.md
  - tests/cases/settings/TC-024_layout-reset-puts-views-back.md
  - tests/cases/shell/TC-010_palette-greys-out-what-cannot-run-and-says-why.md
  - tests/cases/shell/TC-006_palette-on-an-open-call-lists-its-actions.md
  - tests/cases/shell/TC-005_settings-shortcut-and-return-to-where-you-were.md
  - tests/cases/shell/TC-007_palette-on-today-lists-the-days-actions.md
  - tests/cases/shell/TC-009_status-bar-actions-button-opens-the-palette.md
  - tests/cases/shell/TC-014_status-bar-shows-the-profile-and-the-entries.md
  - tests/cases/shell/TC-008_palette-typing-ranks-and-enter-goes-to-settings.md
  - tests/cases/vocabulary/TC-002_vocabulary-opens-from-the-palette-and-the-old-link.md
  - tests/cases/shell/TC-017_top-bar-holds-record-account-and-inbox-on-one-line.md
  - tests/cases/shell/TC-015_narrow-window-opens-the-sidebar-over-the-page.md
---

# S-13: Find your way around: the activity bar, the top bar, panels, layout, the actions palette, the status bar and shortcuts

**Story:** Sam learns the window's frame: the activity bar and its keys, the top bar with Record, his account and the Inbox, the right and bottom panels and where they live, the layout he can reset, the ⌘K palette that acts on whatever is open, and the status bar.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`small`) | the lead (`serve.sh up`) | all |
| Today open (`#today`) | stage 1 (step 1) | 1–7 (the activity bar, the keys, the right panel) |
| A changed panel layout (`window-layout.json`) | stages 6, 7, 8 | reset once, by stage 9 (it was reset in the teardowns of stages 6, 7 and 8 too) |
| Vocabulary in the bottom panel | stage 8 (step 2) | 8–9 (stage 9's Reset is stage 8's teardown) |
| A call open in Calls | stage 10 (step 1) | 10–12 (stage 11 opens Acme weekly sync from the list beside Initech planning; stage 12 starts on it) |
| Today open again, the palette closed | stage 13 (step 1) | 13–16 |
| The browser at a narrow size | stage 18 (steps 3–4) | 18–19 (stage 19 resizes from there; its teardown puts 1201 x 868 back) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `shell/TC-001_activity-bar-items-and-order.md` | — | — |
| 2 | `shell/TC-002_sidebar-expand-and-fold-is-remembered.md` | — | On stage 1's view (`#today`): in step 1 skip the navigation, click "Expand the sidebar" |
| 3 | `shell/TC-003_sidebar-flips-with-the-keyboard.md` | — | On stage 2's view (`#today`, the sidebar folded): in step 1 skip the navigation, press `ControlOrMeta+\` |
| 4 | `shell/TC-004_number-shortcuts-open-the-activities.md` | — | On stage 3's view (`#today`): in step 1 skip the navigation, press `ControlOrMeta+2` |
| 5 | `shell/TC-011_inbox-button-shows-and-hides-the-right-panel.md` | — | On stage 4's view (`#today` after its `ControlOrMeta+1`): skip step 1's navigation, look for "Close the panel" |
| 6 | `shell/TC-012_right-panel-starts-closed-and-keeps-its-last-tab.md` | — | On stage 5's view (`#today`, the panel closed): in step 1 skip the navigation, click "Inbox", then the tab "Vocabulary". In its teardown only click "Close the panel": the layout is reset once, by stage 9 |
| 7 | `shell/TC-013_a-panel-view-moves-to-the-bottom-panel.md` | — | On stage 6's view (`#today`, the panel closed): in step 1 skip the navigation, click "Inbox". In its teardown only click the right panel's "Close the panel" (`[data-panel-toggle="right"]`; the bottom panel stays open on "Claude" and has its own): the layout is reset once, by stage 9 |
| 8 | `settings/TC-023_layout-move-vocabulary-to-bottom.md` | — | Skip its teardown: keep Vocabulary in the bottom panel for stage 9 |
| 9 | `settings/TC-024_layout-reset-puts-views-back.md` | 8 | On stage 8's view (`#settings/layout`, "Bottom" pressed in the row "Vocabulary"): skip step 1, check its expected result on one snapshot; go on with step 2 |
| 10 | `shell/TC-010_palette-greys-out-what-cannot-run-and-says-why.md` | — | — |
| 11 | `shell/TC-006_palette-on-an-open-call-lists-its-actions.md` | — | On stage 10's view (Initech planning open, the calls list beside it): in step 1 skip the navigation, click the row "Acme weekly sync" |
| 12 | `shell/TC-005_settings-shortcut-and-return-to-where-you-were.md` | 11 | On stage 11's view (Acme weekly sync open, picked by a click): reload the page once (a full load of the same `#call/…` URL keeps the call open; without it Settings would reopen on the Layout section stages 8–9 left, not General); skip step 1, check its expected result (the hash) on one snapshot; go on with step 2 |
| 13 | `shell/TC-007_palette-on-today-lists-the-days-actions.md` | — | — |
| 14 | `shell/TC-009_status-bar-actions-button-opens-the-palette.md` | — | On stage 13's view (`#today`, the palette closed): in step 1 skip the navigation, click "Actions" in the status bar |
| 15 | `shell/TC-014_status-bar-shows-the-profile-and-the-entries.md` | — | On stage 14's view (`#today`): skip step 1's navigation, check its expected result on one snapshot |
| 16 | `shell/TC-008_palette-typing-ranks-and-enter-goes-to-settings.md` | — | On stage 15's view (`#today`): in step 1 skip the navigation, press `ControlOrMeta+k` and type `plugins` |
| 17 | `vocabulary/TC-002_vocabulary-opens-from-the-palette-and-the-old-link.md` | — | — |
| 18 | `shell/TC-017_top-bar-holds-record-account-and-inbox-on-one-line.md` | — | Click "Close the panel" (stage 17 left the Vocabulary panel open). Skip its teardown: stage 19 sets its own size |
| 19 | `shell/TC-015_narrow-window-opens-the-sidebar-over-the-page.md` | — | — |

## Notes
- The layout is reset once, by stage 9 (Settings → Layout → Reset): stages 6 and 7 only close the panel, and stage 8 leaves Vocabulary at the bottom for stage 9 to put back. Stage 8 reads only the rows of Vocabulary and Claude, which stages 6–7 don't move.
- The stages walk the window once per place: Today for the bar, the keys and the right panel (1–7), Layout (8–9), Calls for the palette on a call and the return to it (10–12), Today again for the palette and the status bar (13–16), then the old Vocabulary link and the sizes (17–19).
- Keyboard stages use `ControlOrMeta` and need focus outside text boxes.
- Palette stages never run an item that writes or starts Claude Code; they read, type and close.
