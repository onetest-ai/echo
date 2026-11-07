---
id: S-16
title: Setup off the happy path: default folder, model later, broken download, denied permissions
run: agent
platform: macos
env: a Mac where Echo has never been; reset by `tests/tools/clean-mac.sh --yes --app` plus the printed tccutil lines (the user runs those); the release DMG v0.8.2
user_steps:
  - 'Gatekeeper: the user clicks Open at "“Echo” is an app downloaded from the Internet"'
  - 'Network: the user turns Wi-Fi off (or unplugs the cable) for a minute during a model download, then on'
  - 'Permissions: the user runs `tccutil reset All ai.onetest.echo`, restarts Echo, and answers the prompts as the run asks (Don''t Allow / Allow)'
  - 'System Settings: the user enables System Audio Recording for Echo, and confirms the "Screen Recording needed" notification'
  - 'First-run dialogs: the user clicks the button the run names, if the run''s terminal may not click through System Events'
cases:
  - tests/cases/install/TC-003_default-data-folder.md
  - tests/cases/install/TC-005_model-later.md
  - tests/cases/install/TC-006_model-download-interrupted.md
  - tests/cases/install/TC-007_remove-model.md
  - tests/cases/permissions/TC-003_system-audio-denied.md
  - tests/cases/permissions/TC-005_grant-later.md
  - tests/cases/permissions/TC-004_mic-denied.md
  - tests/cases/permissions/TC-007_screen-recording-frames.md
---

# S-16: Setup off the happy path: default folder, model later, broken download, denied permissions

**Environment:** a Mac where Echo has never been; reset by `tests/tools/clean-mac.sh --yes --app` plus the printed
tccutil lines (the user runs those); the release DMG v0.8.2. How the run drives the real install: app_profile.md →
Clean-install runs.

**Story:** The same first run, but the user takes the defaults, postpones the model, loses the network during a
download, removes a model, and refuses permissions before granting them later.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `install/TC-003_default-data-folder.md` | — | `tests/tools/clean-mac.sh` finds nothing; install the DMG as install/TC-001 steps 1–2 |
| 2 | `install/TC-005_model-later.md` | 1 | Start the served window (Clean-install runs) |
| 3 | `install/TC-006_model-download-interrupted.md` | 1 | Pick a catalog model not downloaded yet (`$E model list`); it is the second model stage 4 needs |
| 4 | `install/TC-007_remove-model.md` | 2 | — |
| 5 | `permissions/TC-003_system-audio-denied.md` | 1 | — |
| 6 | `permissions/TC-005_grant-later.md` | 5 | — |
| 7 | `permissions/TC-004_mic-denied.md` | 1 | — |
| 8 | `permissions/TC-007_screen-recording-frames.md` | 1 | Stop the served window at the end |

## Notes
- Every "**The user does:**" step: the run says in its session exactly what will appear and what to do, waits for
  the user, then checks. The run never clicks a system prompt, types a password or changes System Settings.
- The calls with a permission denied and the shared-screen snapshots are recording/TC-020 to TC-022 (S-18).
- Run it only after a reset, when the user says so: it removes the install S-15 left.
