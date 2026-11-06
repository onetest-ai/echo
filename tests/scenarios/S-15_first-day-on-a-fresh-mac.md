---
id: S-15
title: First day on a fresh Mac: install, folder, model, permissions, the CLI and Claude Code
run: person
env: the second Mac: a test user account where Echo has never been (reset between runs: the app, its data and config folders and LaunchAgent removed; its permissions reset by the user), the release DMG
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

**Environment:** the second Mac: a test user account where Echo has never been (reset between runs: the app, its data and config folders and LaunchAgent removed; its permissions reset by the user), the release DMG

**Story:** A new user downloads Echo, installs it, picks where calls live, gets the speech model, grants what Echo asks for, puts the CLI on the PATH, connects Claude Code, and finds Echo there after the next login.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `install/TC-001_dmg-fresh-mac.md` | — | — |
| 2 | `install/TC-002_choose-data-folder.md` | 1 | — |
| 3 | `install/TC-004_model-first-run.md` | 1 | — |
| 4 | `permissions/TC-001_prompts-name-echo.md` | 1 | — |
| 5 | `permissions/TC-002_health-ok.md` | 4 | — |
| 6 | `cli/TC-003_install-cli-tool.md` | 1 | — |
| 7 | `claude/TC-001_connect-plugin.md` | 1 | — |
| 8 | `lifecycle/TC-006_notifications-from-echo.md` | 1 | — |
| 9 | `lifecycle/TC-001_start-at-login.md` | 1 | — |

## Notes
- Stages 4–5 check that the macOS permission prompts appear and name Echo; answering them is the user's (a security setting).
