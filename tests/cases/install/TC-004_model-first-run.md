---
id: TC-004
title: Download the speech model at first run
priority: critical
type: functional
requirements: [F-LIFE-01, F-SET-09]
module: install
run: agent
platform: any
tags: [install, model]
external_id: TC-INST-004
---

# TC-004: Download the speech model at first run

**Module:** Install | **Priority:** Critical | **Type:** Functional

## Preconditions
- A Mac where Echo has never been (app_profile.md → Clean-install runs): `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; otherwise the run stops and asks the user to reset it
- The folder question answered (install/TC-002)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At "Download the speech model now?" read the dialog, then click "Download" (System Events, or **the user does:** click it when the run asks) | The dialog names the suggested model (`parakeet-tdt-0.6b-v3`, 714 MB, in a release that ships NeMo-Speech) with its size |
| 2 | During the download: `$E model list`; Settings → Transcription in the window (served: app_profile.md → Clean-install runs) | Progress in % |
| 3 | Wait until it ends | "Speech model ready" (notification: the user confirms, or `$E model list` marks it in use); the file is in `~/Library/Application Support/echo/models/`, nothing under the data folder's `.echo/models` |

## Expected Final State
The model is downloaded once to the app's folder and used.
