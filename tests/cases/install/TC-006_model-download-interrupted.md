---
id: TC-006
title: An interrupted model download fails cleanly
priority: low
type: functional
requirements: [F-LIFE-01]
module: install
run: agent
platform: macos
tags: [install, model]
external_id: TC-INST-006
user_steps:
  - 'at about 30 % the user turns Wi-Fi off (or unplugs the cable) for a minute when the run asks, then back on'
---

# TC-006: An interrupted model download fails cleanly

**Module:** Install | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo installed and running (S-16); a model not downloaded yet; `E=/Applications/Echo.app/Contents/MacOS/echoctl`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** start the download in Echo's own window → Settings → Transcription; watch `$E model list`. **The user does:** at about 30 % turn the network off for a minute when the run asks, then on | "Model download failed" with a readable reason, or the download resumes |
| 2 | **The user does:** download again from Settings | It completes and the checksum is verified; `$E model list` uses it; no half-downloaded file (`*.part`) was ever in use |

## Expected Final State
A broken download never leaves a broken model.
