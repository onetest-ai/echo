---
id: TC-028
title: General and Health say what is this computer's; Claude Code is the run's, with no home path on the page
priority: medium
type: functional
requirements: [F-SET-19]
module: settings
run: agent
platform: any
data: standard
tags: [settings, health, privacy]
auto: [tests/auto/specs/S-29.spec.mjs]
---

# TC-028: General and Health say what is this computer's; Claude Code is the run's, with no home path on the page

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); shell steps run after `. <dir>/env`
- The standard set is hermetic for Claude Code: its fake `claude` is first on the PATH (`<dir>/bin/claude`) and Claude Code's config folder is `<dir>/claude-config` (`CLAUDE_CONFIG_DIR`), which lists an Echo plugin 0.0.1 for every session

## Test Data

| Field | Value |
|---|---|
| Data folder | `$ECHOCTL_HOME` (`<dir>/home`) |
| Claude Code | `<dir>/bin/claude`, version `2.1.400 (Claude Code, test fake)` |
| Echo plugin | user scope, version 0.0.1 (older than this build) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash: `echo "$ECHOCTL_HOME"; echo "$(dirname "$ECHOCTL_HOME")/bin/claude"; echo "$HOME"` | Three lines: the data folder, the fake's path, the account's home folder (keep them for the next steps) |
| 2 | Navigate to `{{base_url}}/#settings/general` | Under the heading "General" the line "Start at login and the command line are this computer’s, whichever data folder Echo uses; Claude Code is the one Echo’s PATH finds."; the row "Data folder" shows the data folder of step 1 |
| 3 | Navigate to `{{base_url}}/#settings/health`; wait until the "Claude Code" card no longer says "Checking…" (up to 20 s; the page reads again by itself) | Under the heading "Health" the line "What Echo needs from this computer — the same whichever data folder it uses." |
| 4 | Read the "Claude Code" card | Its first row "✓ Claude Code" reads `2.1.400 (Claude Code, test fake) · ` and the fake's path of step 1 (written `~/…` instead when that path is inside the home folder); the row "✓ Echo plugin" reads "Claude Code lists it (every session)."; the row "✗ An older Echo plugin" names `0.0.1 (user)` |
| 5 | Read the text of the whole Health page (`document.querySelector('.set-main').innerText`) | It does not contain the home folder of step 1 followed by `/` (a path under the home folder shows as `~/…`) |

## Expected Final State
Nothing changed: the case only reads.

## Notes
- #104: General and Health showed the real machine with no word that they do (Claude Code's path under the real home folder, the real plugin installs). Kept as the computer's, by design: Start at login, the command line, the macOS permissions, the speech engine. Claude Code follows the run's PATH and `CLAUDE_CONFIG_DIR`, which a test data folder sets; its paths show the home folder as `~`.
- The "In a terminal" row asks the account's own login shell, so it is the computer's: not asserted beyond step 5.
- Step 4's ✓ on "Claude Code" needs the fake to answer `--version` (it does); every set has the fake (plain `small` too, since the 0.8.4 regression).
