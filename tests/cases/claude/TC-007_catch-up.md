---
id: TC-007
title: Calls missed while Echo was quit get their minutes on the next start
priority: medium
type: functional
requirements: [F-MIN-18, F-MIN-14]
module: claude
run: manual
platform: macos
tags: [minutes, auto]
external_id: TC-CLA-006
---

# TC-007: Calls missed while Echo was quit get their minutes on the next start

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- A 4-minute call recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | When its transcript is ready, quit Echo before the minutes' time | — |
| 2 | An hour later start Echo; open a Claude Code session too | Within a minute Echo starts the call's minutes (Show log); "Minutes ready" follows; the session does nothing for it |

## Expected Final State
A missed call is caught up by Echo, not by a session.
