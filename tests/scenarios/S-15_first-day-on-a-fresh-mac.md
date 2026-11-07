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
  - tests/cases/lifecycle/TC-006_notifications-from-echo.md
  - tests/cases/cli/TC-003_install-cli-tool.md
  - tests/cases/claude/TC-001_connect-plugin.md
  - tests/cases/lifecycle/TC-001_start-at-login.md
---

# S-15: First day on a fresh Mac: install, folder, model, permissions, the CLI and Claude Code

**Environment:** a Mac where Echo has never been; reset by `tests/tools/clean-mac.sh --yes --app` plus the printed
tccutil lines (the user runs those); the release DMG v0.8.2. How the run drives the real install: app_profile.md →
Clean-install runs.

**Story:** A new user downloads Echo, installs it, picks where calls live, gets the speech model, grants what Echo
asks for, puts the CLI on the PATH, connects Claude Code, and finds Echo there after the next login.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `install/TC-001_dmg-fresh-mac.md` | — | `tests/tools/clean-mac.sh` finds nothing; download the DMG and set its quarantine flag (the case's Preconditions). Its step 3 runs after stages 2–3 |
| 2 | `install/TC-002_choose-data-folder.md` | 1 | — |
| 3 | `install/TC-004_model-first-run.md` | 2 | — |
| 4 | `permissions/TC-001_prompts-name-echo.md` | 3 | — |
| 5 | `permissions/TC-002_health-ok.md` | 4 | Start the served window (Clean-install runs) |
| 6 | `lifecycle/TC-006_notifications-from-echo.md` | 1 | Its step 1 is the first notification prompt: it may come during stages 3–4 |
| 7 | `cli/TC-003_install-cli-tool.md` | 1 | — |
| 8 | `claude/TC-001_connect-plugin.md` | 1 | — |
| 9 | `lifecycle/TC-001_start-at-login.md` | 1 | Stop the served window; write the resume note. Last: the session ends at the logout |

## Notes
- Every "**The user does:**" step: the run says in its session exactly what will appear and what to do, waits for
  the user, then checks. The run never clicks a system prompt, types a password or changes System Settings.
- Recording a call is not part of this scenario (S-17, S-18).
- Echo stays installed after the run; S-16 starts from a reset.
