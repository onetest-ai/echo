---
id: TC-006
title: Sign-in tokens are private and never logged
priority: high
type: functional
module: privacy
run: manual
platform: macos
tags: [privacy, m365]
external_id: TC-PRIV-006
---

# TC-006: Sign-in tokens are private and never logged

**Module:** Privacy | **Priority:** High | **Type:** Functional

## Preconditions
- Microsoft 365 connected (calendar TC-002)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `ls -l ~/Library/Application\ Support/echo/graph.json` | `-rw-------` |
| 2 | Search `<data folder>/.echo/app.log` for `access_token` and `refresh_token` | No match |

## Expected Final State
Tokens are readable only by the user and never appear in logs.
