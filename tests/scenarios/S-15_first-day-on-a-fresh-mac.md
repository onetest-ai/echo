---
id: S-15
title: First day on a fresh Mac: install, folder, model, permissions, the CLI and Claude Code
run: agent
platform: macos
env: a Mac where Echo has never been; reset by `tests/tools/clean-mac.sh --yes --app` plus the printed tccutil lines (the user runs those); the release DMG v0.8.2
user_steps:
  - 'Gatekeeper: the user clicks Open at "“Echo” is an app downloaded from the Internet"'
  - 'Permissions: the user clicks Allow at the Microphone and System Audio Recording prompts, and tells the run what each prompt names'
  - 'Notifications: the user clicks Allow at the prompt, confirms the banners and their icon, and turns Echo''s notifications off and on in System Settings'
  - 'CLI: the user types the administrator password in Echo''s prompt'
  - 'Login: the user logs out and back in, then restarts the run''s session to resume'
  - 'First-run dialogs: the user clicks the button the run names, if the run''s terminal may not click through System Events'
cases:
  - tests/cases/install/TC-001_dmg-fresh-mac.md
  - tests/cases/install/TC-002_choose-data-folder.md
  - tests/cases/install/TC-004_model-first-run.md
  - tests/cases/permissions/TC-001_prompts-name-echo.md
  - tests/cases/permissions/TC-002_health-ok.md
  - tests/cases/cli/TC-003_install-cli-tool.md
  - tests/cases/claude/TC-001_connect-plugin.md
  - tests/cases/lifecycle/TC-006_notifications-from-echo.md
  - tests/cases/lifecycle/TC-001_start-at-login.md
---

# S-15: First day on a fresh Mac: install, folder, model, permissions, the CLI and Claude Code

**Environment:** a Mac where Echo has never been; reset by `tests/tools/clean-mac.sh --yes --app` plus the printed
tccutil lines (the user runs those); the release DMG v0.8.2. How the run drives the real install: app_profile.md →
Clean-install runs.

**Story:** A new user downloads Echo, installs it, picks where calls live, gets the speech model, grants what Echo
asks for, puts the CLI on the PATH, connects Claude Code, and finds Echo there after the next login.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| `RUN="$(mktemp -d)/S-15"`, `E=/Applications/Echo.app/Contents/MacOS/echoctl`; the release DMG with a browser's quarantine flag (one download) | Prelude | 1 |
| The install: Echo dragged to /Applications, its first start (Gatekeeper's Open) | stage 1, step 2 | 1–9 |
| The first-run answers: the folder `~/Documents/Echo Calls` and the speech model (one download, ~550 MB) | stages 2, 3 | 3–9 (the model: 3 only checks it) |
| The served window on port 48150 (one start) | stage 2, after its step 2 | 2, 3 (Settings → General, → Transcription); stopped before stage 9 |
| The permissions granted at Echo's prompts (Microphone, System Audio Recording) | stage 4, step 2 | 4, 5 |
| Notifications allowed at their first prompt | lifecycle/TC-006 step 1, when the prompt comes (stages 3–4) | 6, 7, 8 |
| The CLI installed (one administrator password) and its "Command-line tool installed" notification | stage 6, step 1 | 6; 8 (its steps 2–3 read that notification) |
| Claude Code connected (its notification) | stage 7, step 1 | 7 |
| "Start Echo at login" on, one logout and login | stage 9 | 9 |

## Prelude

1. `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; else stop and ask the user to reset (app_profile.md → Clean-install runs).
2. `RUN="$(mktemp -d)/S-15"; mkdir -p "$RUN"; E=/Applications/Echo.app/Contents/MacOS/echoctl`; download the DMG and set its quarantine flag (install/TC-001's Preconditions).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `install/TC-001_dmg-fresh-mac.md` | — | The DMG is the prelude's. Its step 3 runs after stages 2–3 (the welcome questions come first) |
| 2 | `install/TC-002_choose-data-folder.md` | 1 | **After its step 2:** start the served window once (`ECHOCTL_SERVE_IPC=1 nohup $E window --serve 48150 > "$RUN/serve.log" 2>&1 & echo $! > "$RUN/pid"`, app_profile.md → Clean-install runs) and load `http://127.0.0.1:48150/`: its step 3 and stage 3's step 2 use it |
| 3 | `install/TC-004_model-first-run.md` | 2 | On stage 2's served window. If the notifications prompt comes now (the model-ready one), answer it as stage 8's step 1 |
| 4 | `permissions/TC-001_prompts-name-echo.md` | 3 | If the notifications prompt comes now, answer it as stage 8's step 1 |
| 5 | `permissions/TC-002_health-ok.md` | 4 | — |
| 6 | `cli/TC-003_install-cli-tool.md` | 1 | Its step 1's notification is also stage 8's steps 2–3: when the user has typed the password, they bring another app to the front (a banner; else it is only in Notification Center), confirm its icon, and click it — the Echo window opens. Note both for stage 8 |
| 7 | `claude/TC-001_connect-plugin.md` | 1 | — |
| 8 | `lifecycle/TC-006_notifications-from-echo.md` | 6 | Its step 1 was answered when the prompt came (stages 3–4) and its steps 2–3 on stage 6's notification: record what the user confirmed then, don't install or connect anything again. Step 4's trigger: `$E notify "Echo" "Notifications test"` (posted as the app posts them): once before notifications are turned off (the user confirms a banner: the trigger works), once after |
| 9 | `lifecycle/TC-001_start-at-login.md` | 1 | Stop the served window (`kill "$(cat "$RUN/pid")"`); write the resume note. Last: the session ends at the logout |

## Notes
- Every "**The user does:**" step: the run says in its session exactly what will appear and what to do, waits for
  the user, then checks. The run never clicks a system prompt, types a password or changes System Settings.
- **One of each:** one DMG download, one install and first start, one speech-model download, one served window
  (started where stage 2 first needs it, not at stage 5: stages 2–3 read Settings in it), one administrator password, one
  Connect, one logout. Notifications are judged on the ones the stages make anyway (the CLI's), so nothing is
  installed or connected twice to trigger one; with notifications off, `$E notify` makes the test one.
- If `$E notify` shows nothing while notifications are still on (a CLI process may not post as the app), stage 8's
  step 4 triggers one as its step 2 says (Connect or Install… again) and the report says so.
- Recording a call is not part of this scenario (S-17, S-18).
- Echo stays installed after the run; S-16 starts from a reset.
