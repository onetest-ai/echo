---
id: TC-001
title: Connect Claude Code installs Echo's plugin
priority: high
type: functional
module: claude
run: manual
platform: macos
tags: [plugin, setup]
external_id: TC-CLA-001
---

# TC-001: Connect Claude Code installs Echo's plugin

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- Echo's plugin not installed

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar → Claude Code → "Connect Claude Code (install the plugin)" | A "Claude Code connected" notification |
| 2 | Run `claude plugin list` | `echo@onetest` is listed |
| 3 | In a new session type `/echo:` | `process-call`, `live`, `record` and `doctor` are offered |

## Expected Final State
Echo's plugin is installed and its commands are offered in a new session.
