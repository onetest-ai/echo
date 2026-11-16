---
id: TC-005
title: Skip the model at first run and download it later
priority: high
type: functional
requirements: [F-LIFE-01, F-SET-09]
module: install
run: agent
platform: any
tags: [install, model]
external_id: TC-INST-005
---

# TC-005: Skip the model at first run and download it later

**Module:** Install | **Priority:** High | **Type:** Functional

## Preconditions
- A Mac where Echo has never been (app_profile.md → Clean-install runs): `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; otherwise the run stops and asks the user to reset it
- The folder question answered (install/TC-003)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Click "Later" at the model prompt (System Events, or **the user does:** click it when the run asks) | Echo keeps running (`$E status`); `$E model list` has no model in use; Settings → Transcription offers the download |
| 2 | **The user does:** Echo's own window → Settings → Transcription → the recommended model → "Download (… MB)" when the run asks; the run follows `$E model list` | It downloads (progress, then ready) |
| 3 | **The user does:** "Use this model" there | It is in use (`$E model list`) |

## Expected Final State
A model can be added any time from Settings.

## Notes
- Recording before a model exists is in S-18 (it needs a call).
