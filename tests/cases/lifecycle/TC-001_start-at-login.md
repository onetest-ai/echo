---
id: TC-001
title: Echo starts at login when set to
priority: high
type: functional
requirements: [F-LIFE-02]
module: lifecycle
run: agent
platform: any
tags: [lifecycle, login]
external_id: TC-LIFE-001
user_steps:
  - 'the user logs out and back in, then restarts the run''s Claude Code session, when the run asks'
---

# TC-001: Echo starts at login when set to

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed and running; `E=/Applications/Echo.app/Contents/MacOS/echoctl`
- "Start Echo at login" on: **the user does:** tick it in Echo's own window → Settings → General (it is off after a fresh install) ( `$E app` shows `"autostart": true` and `~/Library/LaunchAgents/ai.onetest.echo.plist` exists)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Write the resume note `tests/reports/S-15-resume.md` (stages done, their results, what to check next). **The user does:** log out and back in, then start the run's session again and tell it to resume | — |
| 2 | After the login, without opening anything: `pgrep -fl /Applications/Echo.app/Contents/MacOS/echoctl`; `$E status`; `launchctl print gui/$(id -u)/ai.onetest.echo` | Echo runs (started by launchd), the tray is there (the user confirms the octopus); no first-run dialog appeared |

## Expected Final State
Echo starts at login by itself.
