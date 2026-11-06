---
id: S-16
title: Setup off the happy path: default folder, model later, broken download, denied permissions
run: person
env: the second Mac: a test user account where Echo has never been (reset between runs: the app, its data and config folders and LaunchAgent removed; its permissions reset by the user), the release DMG
cases:
  - tests/cases/install/TC-003_default-data-folder.md
  - tests/cases/install/TC-005_model-later.md
  - tests/cases/install/TC-006_model-download-interrupted.md
  - tests/cases/install/TC-007_remove-model.md
  - tests/cases/permissions/TC-003_system-audio-denied.md
  - tests/cases/permissions/TC-004_mic-denied.md
  - tests/cases/permissions/TC-005_grant-later.md
  - tests/cases/permissions/TC-007_screen-recording-frames.md
---

# S-16: Setup off the happy path: default folder, model later, broken download, denied permissions

**Environment:** the second Mac: a test user account where Echo has never been (reset between runs: the app, its data and config folders and LaunchAgent removed; its permissions reset by the user), the release DMG

**Story:** The same first run, but the user takes the defaults, postpones the model, loses Wi-Fi during a download, removes a model, and refuses permissions before granting them later.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `install/TC-003_default-data-folder.md` | — | — |
| 2 | `install/TC-005_model-later.md` | — | — |
| 3 | `install/TC-006_model-download-interrupted.md` | — | — |
| 4 | `install/TC-007_remove-model.md` | — | — |
| 5 | `permissions/TC-003_system-audio-denied.md` | — | — |
| 6 | `permissions/TC-004_mic-denied.md` | — | — |
| 7 | `permissions/TC-005_grant-later.md` | 5 | — |
| 8 | `permissions/TC-007_screen-recording-frames.md` | — | — |

## Notes
- Permission stages need Echo's permissions reset first (`tccutil reset All ai.onetest.echo`, by the user) and the prompts answered by the user.
