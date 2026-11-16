---
id: TC-003
title: Use the default data folder
priority: high
type: functional
requirements: [F-LIFE-01]
module: install
run: agent
platform: any
tags: [install, data-folder]
external_id: TC-INST-003
---

# TC-003: Use the default data folder

**Module:** Install | **Priority:** High | **Type:** Functional

## Preconditions
- A Mac where Echo has never been (app_profile.md → Clean-install runs): `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; otherwise the run stops and asks the user to reset it
- Echo just opened for the first time (install/TC-001's steps 1–2)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At the folder question click "Use default" (System Events, or **the user does:** click it when the run asks) | — |
| 2 | `$E app`; Settings → General in the window (served: app_profile.md → Clean-install runs) | `home` is `~/Echo`; Settings shows `~/Echo`; `~/Echo/calls` and `~/Echo/.echo` exist |

## Expected Final State
The default folder is used.
