---
id: S-21
title: Living with Echo: login, quit, one instance, another folder, the native window, updates, offline, the network
run: person
env: this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists
cases:
  - tests/cases/lifecycle/TC-002_start-at-login-off.md
  - tests/cases/lifecycle/TC-003_quit.md
  - tests/cases/cli/TC-006_app-start-after-quit.md
  - tests/cases/lifecycle/TC-004_single-instance.md
  - tests/cases/lifecycle/TC-005_change-data-folder.md
  - tests/cases/window/TC-011_native-window-dock.md
  - tests/cases/window/TC-015_appearance-follows-system.md
  - tests/cases/lifecycle/TC-007_update.md
  - tests/cases/lifecycle/TC-008_no-update-while-recording.md
  - tests/cases/lifecycle/TC-009_up-to-date.md
  - tests/cases/privacy/TC-004_offline-transcription.md
  - tests/cases/privacy/TC-005_network-destinations.md
---

# S-21: Living with Echo: login, quit, one instance, another folder, the native window, updates, offline, the network

**Environment:** this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists

**Story:** Over days of use: Echo starting at login or not, quitting and coming back, a second start, moving the data folder, the native window and its appearance, updates offered, refused during a recording and absent when up to date, working offline, and the network it talks to.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `lifecycle/TC-002_start-at-login-off.md` | — | — |
| 2 | `lifecycle/TC-003_quit.md` | — | — |
| 3 | `cli/TC-006_app-start-after-quit.md` | — | — |
| 4 | `lifecycle/TC-004_single-instance.md` | — | — |
| 5 | `lifecycle/TC-005_change-data-folder.md` | — | — |
| 6 | `window/TC-011_native-window-dock.md` | — | — |
| 7 | `window/TC-015_appearance-follows-system.md` | — | — |
| 8 | `lifecycle/TC-007_update.md` | — | — |
| 9 | `lifecycle/TC-008_no-update-while-recording.md` | — | — |
| 10 | `lifecycle/TC-009_up-to-date.md` | — | — |
| 11 | `privacy/TC-004_offline-transcription.md` | — | — |
| 12 | `privacy/TC-005_network-destinations.md` | — | — |

## Notes
- Updates need a release server the build can be pointed at (not yet: the URL is fixed to GitHub).
