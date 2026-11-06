---
id: TC-003
title: Settings installs echoctl on the PATH
priority: high
type: functional
requirements: [F-SET-05]
module: cli
run: person
platform: macos
tags: [cli, install]
external_id: TC-CLI-001
---

# TC-003: Settings installs echoctl on the PATH

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed in /Applications and running; `echoctl` not on the PATH yet

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar octopus → Settings → "Install command-line tool (Echo)…"; enter the administrator password | A "Command-line tool installed" notification |
| 2 | In a new terminal run `which echoctl` | `/usr/local/bin/echoctl`, a link into `/Applications/Echo.app` |
| 3 | Run `echoctl status` | JSON with the running app's watcher and state |

## Expected Final State
`echoctl` is on the PATH, points into the app, and talks to the running app.
