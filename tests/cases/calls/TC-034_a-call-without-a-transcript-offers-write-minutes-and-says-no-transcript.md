---
id: TC-034
title: A call whose transcription failed, or that has audio only, says No minutes yet and no transcript
priority: medium
type: functional
requirements: [F-MIN-06, F-CALLS-02]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, status]
---

# TC-034: A call whose transcription failed, or that has audio only, says No minutes yet and no transcript

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Status | Minutes tab | Transcript tab |
|---|---|---|---|
| Umbrella intro call (`umbrella-intro-call`) | failed | No minutes yet · Write minutes | No transcript for this call. |
| Recording (`recording`) | audio | No minutes yet · Write minutes | No transcript for this call. |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Umbrella intro call, then the tab "Minutes" | The heading "No minutes yet" and a button "Write minutes" (do not press it) |
| 2 | Click the tab "Transcript" | The text "No transcript for this call."; no turns, no Find box |
| 3 | Click the row Recording | The Transcript tab (it stays) says "No transcript for this call." |
| 4 | Click the tab "Minutes" | The heading "No minutes yet" and a button "Write minutes" |

## Expected Final State
Recording's Minutes tab is open with Write minutes.

## Notes
- Creates or changes nothing.
- Seen on the data (2026-11-06): the Minutes box of these two calls also says "The transcript is ready.", although there is no transcript. This case does not assert that sentence; see the report.
