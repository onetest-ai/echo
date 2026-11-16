---
id: TC-002
title: Choose the data folder at first launch
priority: critical
type: functional
requirements: [F-LIFE-01]
module: install
run: agent
platform: any
tags: [install, data-folder]
external_id: TC-INST-002
---

# TC-002: Choose the data folder at first launch

**Module:** Install | **Priority:** Critical | **Type:** Functional

## Preconditions
- A Mac where Echo has never been (app_profile.md → Clean-install runs): `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; otherwise the run stops and asks the user to reset it
- Echo just opened for the first time (install/TC-001)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At "Where should call transcripts and notes be kept?" click "Choose folder…" (the run clicks it through System Events; without that permission, **the user does:** click it when the run asks) | A folder picker |
| 2 | In the picker, Cmd+Shift+G, type `~/Documents/Echo Calls` (a space in the name), Return, Choose (the same way) | Accepted |
| 3 | `$E app`; Settings → General in the window (served: app_profile.md → Clean-install runs) | `home` is `~/Documents/Echo Calls` (`home_source` "chosen"); Settings shows that folder |
| 4 | `ls -A ~/Documents/Echo\ Calls`; `ls ~/Echo` | `calls/` and `.echo/` are there; `~/Echo` doesn't exist |

## Expected Final State
Echo keeps everything in the folder chosen.
