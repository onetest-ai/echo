---
id: TC-005
title: Skip the model at first run and download it later
priority: high
type: functional
requirements: [F-LIFE-01, F-SET-09]
module: install
run: person
platform: any
tags: [install, model]
external_id: TC-INST-005
---

# TC-005: Skip the model at first run and download it later

**Module:** Install | **Priority:** High | **Type:** Functional

## Preconditions
- A fresh Mac account (tests/README.md → Test machines): no Echo settings, data folder or permissions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Click "Later" at the model prompt | Echo runs; recording works, transcription waits for a model |
| 2 | Menu → Speech models → the recommended one → "Download (… MB)" | It downloads |
| 3 | "Use this model" | It is in use |

## Expected Final State
A model can be added any time from the menu.
