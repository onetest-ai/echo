---
id: TC-003
title: Use the default data folder
priority: high
type: functional
module: install
run: manual
platform: any
tags: [install, data-folder]
external_id: TC-INST-003
---

# TC-003: Use the default data folder

**Module:** Install | **Priority:** High | **Type:** Functional

## Preconditions
- A fresh Mac account (tests/README.md → Test machines): no Echo settings, data folder or permissions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At the folder question click "Use default" | — |
| 2 | Menu → Settings | Shows `~/Echo` |

## Expected Final State
The default folder is used.
