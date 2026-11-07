---
id: S-21
title: Living with Echo: quit, one instance, another folder, appearance; login, updates, the native window, offline, the network
run: agent
platform: macos
data: small
env: this Mac: the dev build run as a second app beside the user's own Echo, on its own test data folder and config (never `~/echo`, never `app start`), `--scheduled manual`; the window served on the data
cases:
  - tests/cases/lifecycle/TC-004_single-instance.md
  - tests/cases/lifecycle/TC-003_quit.md
  - tests/cases/lifecycle/TC-005_change-data-folder.md
  - tests/cases/window/TC-015_appearance-follows-system.md
  - tests/cases/lifecycle/TC-002_start-at-login-off.md
  - tests/cases/cli/TC-006_app-start-after-quit.md
  - tests/cases/window/TC-011_native-window-dock.md
  - tests/cases/lifecycle/TC-007_update.md
  - tests/cases/lifecycle/TC-008_no-update-while-recording.md
  - tests/cases/lifecycle/TC-009_up-to-date.md
  - tests/cases/privacy/TC-004_offline-transcription.md
  - tests/cases/privacy/TC-005_network-destinations.md
---

# S-21: Living with Echo

**Environment:** this Mac: the dev build run as a second app beside the user's own Echo, on its own test data and config (never `~/echo`); the window served on the data for stage 4.

**Data:** `small`, hermetic: `tests/fixtures/make_data.py <dir> --ai fake`; the speech model linked into its config and TestCall for stage 3's call.

**Story:** Over days of use: a second start refused, quitting and coming back without first-run questions, moving the data folder, the window following the system's light and dark; then what needs an installed Echo, a login or the network: start at login, `app start`, the native window and its Dock icon, updates, offline, the network it talks to.

**Length:** about 5 minutes for stages 1–4.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `lifecycle/TC-004_single-instance.md` | — | Start the dev app |
| 2 | `lifecycle/TC-003_quit.md` | — | — |
| 3 | `lifecycle/TC-005_change-data-folder.md` | — | — |
| 4 | `window/TC-015_appearance-follows-system.md` | — | Serve the window; load `{{base_url}}/` |
| 5 | `lifecycle/TC-002_start-at-login-off.md` | — | **person** |
| 6 | `cli/TC-006_app-start-after-quit.md` | — | **person** |
| 7 | `window/TC-011_native-window-dock.md` | — | **person** |
| 8 | `lifecycle/TC-007_update.md` | — | **person** — needs a newer release (the URL is fixed to GitHub) |
| 9 | `lifecycle/TC-008_no-update-while-recording.md` | — | **person** |
| 10 | `lifecycle/TC-009_up-to-date.md` | — | **person** |
| 11 | `privacy/TC-004_offline-transcription.md` | — | **person** |
| 12 | `privacy/TC-005_network-destinations.md` | — | **person** |

## Notes
- **person** stages are skipped by an agent run (the lead lists them as skipped (person)): each case's Notes say why — mostly that on a dev Mac the installed Echo, its LaunchAgent and its tray are the user's own.
- Run 2026-11-07 on the dev Mac: stages 1–4 PASS; stage 3 found the dev build's restart going to the installed Echo.app (reported).
