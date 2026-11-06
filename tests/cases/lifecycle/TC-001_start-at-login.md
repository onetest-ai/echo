---
id: TC-001
title: Echo starts at login when set to
priority: high
type: functional
requirements: [F-LIFE-02]
module: lifecycle
run: manual
platform: any
tags: [lifecycle, login]
external_id: TC-LIFE-001
---

# TC-001: Echo starts at login when set to

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed and running
- Settings → "Start Echo at login" ticked

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Log out and back in | Echo is in the menu bar (tray) without opening anything; no first-run dialogs |

## Expected Final State
Echo starts quietly at login.
