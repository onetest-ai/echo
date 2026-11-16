---
id: S-16
title: Setup off the happy path: default folder, model later, broken download, denied permissions
run: agent
platform: macos
env: a Mac where Echo has never been; reset by `tests/tools/clean-mac.sh --yes --app` plus the printed tccutil lines (the user runs those); the release DMG v0.8.2
user_steps:
  - 'Gatekeeper: the user clicks Open at "“Echo” is an app downloaded from the Internet"'
  - 'Network: the user turns Wi-Fi off (or unplugs the cable) for a minute during a model download, then on'
  - 'Permissions: the user runs `tccutil reset All ai.onetest.echo` once, restarts Echo, and clicks Don''t Allow at both prompts (Microphone and System Audio Recording) when the run asks'
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

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| `RUN="$(mktemp -d)/S-16"`, `E=/Applications/Echo.app/Contents/MacOS/echoctl`; the release DMG with a browser's quarantine flag (one download) | Prelude | 1 |
| The install and first start (install/TC-001 steps 1–2) | stage 1, before its step 1 | 1–8 |
| The served window on port 48150 (one start) | stage 1, after its step 1 | 1, 2, 8; stopped at the end |
| Model 1: the recommended model, downloaded later from Settings and in use | stage 2, steps 2–3 | 2, 4 (the one in use) |
| Model 2: a catalog model downloaded with the network cut, then again | stage 3 | 3, 4 (the unused one it removes) |
| One permission reset and restart: both prompts answered Don't Allow | stage 5, step 1 | 5; 6 (system audio granted later); 7 (the microphone stays denied) |

## Prelude

1. `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; else stop and ask the user to reset (app_profile.md → Clean-install runs).
2. `RUN="$(mktemp -d)/S-16"; mkdir -p "$RUN"; E=/Applications/Echo.app/Contents/MacOS/echoctl`; download the DMG and set its quarantine flag (install/TC-001's Preconditions).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `install/TC-003_default-data-folder.md` | — | Install the prelude's DMG as install/TC-001 steps 1–2. **After its step 1:** start the served window once (`ECHOCTL_SERVE_IPC=1 nohup $E window --serve 48150 > "$RUN/serve.log" 2>&1 & echo $! > "$RUN/pid"`) and load `http://127.0.0.1:48150/`: its step 2 uses it |
| 2 | `install/TC-005_model-later.md` | 1 | Its step 1's prompt comes right after stage 1's folder question. On the served window |
| 3 | `install/TC-006_model-download-interrupted.md` | 1 | Pick a catalog model not downloaded yet (`$E model list`): it is the second model stage 4 needs |
| 4 | `install/TC-007_remove-model.md` | 2, 3 | Its two models are stages 2 and 3's: nothing is downloaded for it |
| 5 | `permissions/TC-003_system-audio-denied.md` | 1 | Its step 1 is the scenario's one reset: at the prompts the user clicks Don't Allow at **both** Microphone and System Audio Recording (stage 7's microphone too) |
| 6 | `permissions/TC-005_grant-later.md` | 5 | The microphone stays denied (stage 7): its step 2 sees system audio ✓ and only the microphone missing |
| 7 | `permissions/TC-004_mic-denied.md` | 5, 6 | Skip its step 1: the microphone is denied since stage 5's reset and system audio granted since stage 6 |
| 8 | `permissions/TC-007_screen-recording-frames.md` | 1 | Stop the served window at the end (`kill "$(cat "$RUN/pid")"`) |

## Notes
- Every "**The user does:**" step: the run says in its session exactly what will appear and what to do, waits for
  the user, then checks. The run never clicks a system prompt, types a password or changes System Settings.
- **One of each:** one DMG download, one install, one served window (started where stage 1 first needs it, not at
  stage 2), two model downloads (stage 4 removes one of them), and one permission reset with one restart of Echo
  (before 2026-11-08 stages 5 and 7 each reset and restarted: denying both at once and granting system audio in
  between reaches stage 7's state without the second).
- The calls with a permission denied and the shared-screen snapshots are recording/TC-020 to TC-022 (S-18).
- Run it only after a reset, when the user says so: it removes the install S-15 left.
