---
id: TC-018
title: The Screens tab shows each frame with its time, and a click opens it large
priority: medium
type: functional
requirements: [F-CALLS-15]
module: calls
run: agent
platform: any
data: standard
tags: [calls, screens]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-018: The Screens tab shows each frame with its time, and a click opens it large

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Frames |
|---|---|
| Acme weekly sync (Today) | 1:05 and 5:07 (`frames/01m05s.png`, `frames/05m07s.png`) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync, then the tab "Screens" | Two buttons `.frame` are shown, labelled "1:05" and "5:07" |
| 2 | Read the `alt` of the first image | "Shared screen at 1:05" |
| 3 | Click the first frame | An overlay `.lightbox` shows one image with `alt="Shared screen" |
| 4 | Press Escape | The overlay is gone; the two frames are still listed |

## Expected Final State
The Screens tab shows two frames and no overlay.

## Notes
- Creates or changes nothing.
