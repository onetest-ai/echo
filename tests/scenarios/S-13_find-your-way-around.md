---
id: S-13
title: Find your way around: the activity bar, the top bar, panels, layout, the actions palette, the status bar and shortcuts
run: agent
data: small
cases:
  - tests/cases/shell/TC-001_activity-bar-items-and-order.md
  - tests/cases/shell/TC-002_sidebar-expand-and-fold-is-remembered.md
  - tests/cases/shell/TC-017_top-bar-holds-record-account-and-inbox-on-one-line.md
  - tests/cases/shell/TC-003_sidebar-flips-with-the-keyboard.md
  - tests/cases/shell/TC-004_number-shortcuts-open-the-activities.md
  - tests/cases/shell/TC-005_settings-shortcut-and-return-to-where-you-were.md
  - tests/cases/shell/TC-011_inbox-button-shows-and-hides-the-right-panel.md
  - tests/cases/shell/TC-012_right-panel-starts-closed-and-keeps-its-last-tab.md
  - tests/cases/shell/TC-013_a-panel-view-moves-to-the-bottom-panel.md
  - tests/cases/settings/TC-023_layout-move-vocabulary-to-bottom.md
  - tests/cases/settings/TC-024_layout-reset-puts-views-back.md
  - tests/cases/shell/TC-007_palette-on-today-lists-the-days-actions.md
  - tests/cases/shell/TC-006_palette-on-an-open-call-lists-its-actions.md
  - tests/cases/shell/TC-008_palette-typing-ranks-and-enter-goes-to-settings.md
  - tests/cases/shell/TC-009_status-bar-actions-button-opens-the-palette.md
  - tests/cases/shell/TC-010_palette-greys-out-what-cannot-run-and-says-why.md
  - tests/cases/vocabulary/TC-002_vocabulary-opens-from-the-palette-and-the-old-link.md
  - tests/cases/shell/TC-014_status-bar-shows-the-profile-and-the-entries.md
  - tests/cases/shell/TC-015_narrow-window-opens-the-sidebar-over-the-page.md
---

# S-13: Find your way around: the activity bar, the top bar, panels, layout, the actions palette, the status bar and shortcuts

**Story:** Sam learns the window's frame: the activity bar and its keys, the top bar with Record, his account and the Inbox, the right and bottom panels and where they live, the layout he can reset, the ⌘K palette that acts on whatever is open, and the status bar.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `shell/TC-001_activity-bar-items-and-order.md` | — | — |
| 2 | `shell/TC-002_sidebar-expand-and-fold-is-remembered.md` | — | — |
| 3 | `shell/TC-017_top-bar-holds-record-account-and-inbox-on-one-line.md` | — | — |
| 4 | `shell/TC-003_sidebar-flips-with-the-keyboard.md` | — | — |
| 5 | `shell/TC-004_number-shortcuts-open-the-activities.md` | — | — |
| 6 | `shell/TC-005_settings-shortcut-and-return-to-where-you-were.md` | — | — |
| 7 | `shell/TC-011_inbox-button-shows-and-hides-the-right-panel.md` | — | — |
| 8 | `shell/TC-012_right-panel-starts-closed-and-keeps-its-last-tab.md` | — | — |
| 9 | `shell/TC-013_a-panel-view-moves-to-the-bottom-panel.md` | — | — |
| 10 | `settings/TC-023_layout-move-vocabulary-to-bottom.md` | — | — |
| 11 | `settings/TC-024_layout-reset-puts-views-back.md` | — | — |
| 12 | `shell/TC-007_palette-on-today-lists-the-days-actions.md` | — | — |
| 13 | `shell/TC-006_palette-on-an-open-call-lists-its-actions.md` | — | — |
| 14 | `shell/TC-008_palette-typing-ranks-and-enter-goes-to-settings.md` | — | — |
| 15 | `shell/TC-009_status-bar-actions-button-opens-the-palette.md` | — | — |
| 16 | `shell/TC-010_palette-greys-out-what-cannot-run-and-says-why.md` | — | — |
| 17 | `vocabulary/TC-002_vocabulary-opens-from-the-palette-and-the-old-link.md` | — | — |
| 18 | `shell/TC-014_status-bar-shows-the-profile-and-the-entries.md` | — | — |
| 19 | `shell/TC-015_narrow-window-opens-the-sidebar-over-the-page.md` | — | — |

## Notes
- Panel stages reset the layout in their teardown (`Settings -> Layout -> Reset`), so the order does not matter for the data; the order follows a first walk through the window.
- Keyboard stages use `ControlOrMeta` and need focus outside text boxes.
- Palette stages never run an item that writes or starts Claude Code; they read, type and close.
