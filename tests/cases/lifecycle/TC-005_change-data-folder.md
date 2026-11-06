---
id: TC-005
title: Changing the data folder restarts on it and leaves the old one alone
priority: low
type: functional
requirements: [F-LIFE-05]
module: lifecycle
run: manual
platform: macos
tags: [lifecycle, data-folder]
external_id: TC-LIFE-005
---

# TC-005: Changing the data folder restarts on it and leaves the old one alone

**Module:** Lifecycle | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo installed and running
- Some recorded calls

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → Settings → "Change data folder…" → a new empty folder | Echo restarts on it |
| 2 | Record a call | It goes to the new folder |
| 3 | Look in the old folder | Its calls are untouched |

## Expected Final State
The data folder can move without losing or mixing calls.
