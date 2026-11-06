---
id: TC-006
title: An interrupted model download fails cleanly
priority: low
type: functional
requirements: [F-LIFE-01]
module: install
run: person
platform: macos
tags: [install, model]
external_id: TC-INST-006
---

# TC-006: An interrupted model download fails cleanly

**Module:** Install | **Priority:** Low | **Type:** Functional

## Preconditions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Start a model download; at about 30 % turn Wi-Fi off for a minute | "Model download failed" with a readable reason, or the download resumes |
| 2 | Download again from the menu | It completes; no half-downloaded file was ever used |

## Expected Final State
A broken download never leaves a broken model.
