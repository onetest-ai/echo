---
id: TC-034
title: A call whose transcription failed, or that has audio only, says why there is no transcript (and, after a failure, how to transcribe it again), not Write minutes
priority: medium
type: functional
requirements: [F-MIN-21, F-CALLS-02]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, status]
---

# TC-034: A call whose transcription failed, or that has audio only, says why there is no transcript (and, after a failure, how to transcribe it again), not Write minutes

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Status | Minutes tab | Transcript tab |
|---|---|---|---|
| Umbrella intro call (`umbrella-intro-call`) | failed | No transcript, so no minutes · the error · `echoctl transcribe …` | No transcript for this call. |
| Recording (`recording`) | audio | No transcript, so no minutes · not transcribed yet | No transcript for this call. |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Umbrella intro call, then the tab "Minutes" | The heading "No transcript, so no minutes"; "Transcribing this call failed: the speech model could not be loaded: out of memory"; "Transcribe it again in a terminal:" with `echoctl transcribe <its folder>` and "Copy"; no "Write minutes" and no "The transcript is ready." |
| 2 | Click the tab "Transcript" | The text "No transcript for this call."; no turns, no Find box |
| 3 | Click the row Recording | The Transcript tab (it stays) says "No transcript for this call." |
| 4 | Click the tab "Minutes" | The heading "No transcript, so no minutes"; "This recording isn't transcribed yet."; no command and no "Write minutes" |

## Expected Final State
Recording's Minutes tab is open, saying it isn't transcribed yet.

## Notes
- Creates or changes nothing.
- Was "No minutes yet" / "The transcript is ready." with Write minutes until #116.
