---
id: TC-001
title: Connect Claude Code installs Echo's plugin
priority: high
type: functional
requirements: [F-CC-06]
module: claude
run: agent
platform: macos
tags: [plugin, setup]
external_id: TC-CLA-001
---

# TC-001: Connect Claude Code installs Echo's plugin

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Claude Code installed and signed in on this account; Echo running; `E=/Applications/Echo.app/Contents/MacOS/echoctl`
- Echo's plugin not installed (`claude plugin list` has no `echo@onetest`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** Echo's own window → Settings → General → Claude Code plugin → "Connect" when the run asks | A "Claude Code connected" notification (in Notification Center if Echo's window was in front: the user confirms); Settings says "Installed ✓" |
| 2 | `$E claude`; in the data folder (`$E app` → `home`) run `claude plugin list` | `$E claude`: `plugin_installed` true, project scope in the data folder; `echo@onetest` is listed there |
| 3 | List the installed plugin's skills: the folder Claude Code installed it in (`installed_plugins.json` → `installPath`) | `process-call`, `record` and `doctor` are there (with `manage`, `microsoft-365`, `project-sync`, `brain`) (the `/echo:` menu in a new interactive session offers them: the user may check it) |

## Expected Final State
Claude Code lists Echo's plugin and its commands.
