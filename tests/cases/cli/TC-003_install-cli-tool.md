---
id: TC-003
title: Settings installs echoctl on the PATH
priority: high
type: functional
requirements: [F-SET-05]
module: cli
run: agent
platform: macos
tags: [cli, install]
external_id: TC-CLI-001
user_steps:
  - 'the user types the administrator password in Echo''s prompt when the run asks'
---

# TC-003: Settings installs echoctl on the PATH

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed in /Applications and running; `echoctl` not on the PATH yet (`command -v echoctl` finds nothing)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** Echo's own window → Settings → General → Command line → "Install…". **The user does:** type the administrator password in the prompt "Echo wants to add its command-line tool to /usr/local/bin." when the run asks | A "Command-line tool installed" notification (the user confirms); Settings shows it installed |
| 2 | In a new shell run `which echoctl` and `readlink /usr/local/bin/echoctl` | `/usr/local/bin/echoctl`, a link into `/Applications/Echo.app/Contents/MacOS/echoctl` |
| 3 | Run `echoctl status` | JSON with the running app's watcher and state |

## Expected Final State
echoctl is on the PATH and talks to the running app.
