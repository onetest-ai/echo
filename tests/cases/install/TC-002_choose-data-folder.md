---
id: TC-002
title: Choose the data folder at first launch
priority: critical
type: functional
requirements: [F-LIFE-01]
module: install
run: person
platform: any
tags: [install, data-folder]
external_id: TC-INST-002
---

# TC-002: Choose the data folder at first launch

**Module:** Install | **Priority:** Critical | **Type:** Functional

## Preconditions
- A fresh Mac account (tests/README.md → Test machines): no Echo settings, data folder or permissions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At "Where should call transcripts and notes be kept?" click "Choose folder…" | A folder picker |
| 2 | Pick `~/Documents/Echo Calls` (a space in the name) | Accepted |
| 3 | Menu → Settings | Shows that folder |
| 4 | After the first call, look in it | `calls/` and `.echo/` are there; nothing in `~/Echo` |

## Expected Final State
Echo keeps everything in the folder chosen.
