---
id: TC-015
title: The window follows the system's light and dark appearance
priority: low
type: functional
module: window
run: manual
platform: macos
tags: [appearance]
external_id: TC-WIN-010
---

# TC-015: The window follows the system's light and dark appearance

**Module:** Window | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo window open; Settings → Appearance set to follow the system

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Switch macOS to Dark | The window turns dark; all text stays readable |
| 2 | Switch back to Light | The window turns light |

## Expected Final State
The window follows the system appearance both ways.
