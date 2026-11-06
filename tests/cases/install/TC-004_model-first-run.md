---
id: TC-004
title: Download the speech model at first run
priority: critical
type: functional
module: install
run: manual
platform: any
tags: [install, model]
external_id: TC-INST-004
---

# TC-004: Download the speech model at first run

**Module:** Install | **Priority:** Critical | **Type:** Functional

## Preconditions
- A fresh Mac account (tests/README.md → Test machines): no Echo settings, data folder or permissions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At "Download the speech model now?" click "Download" | The suggested model (on Apple Silicon `large-v3-turbo-q5_0`) with its size |
| 2 | Menu → Speech models during the download | Progress in % |
| 3 | Wait | "Speech model ready"; the model marked in use; its file under `~/Library/Application Support/echo/models/`, not in the data folder |

## Expected Final State
The model is downloaded once to the app's folder and used.
