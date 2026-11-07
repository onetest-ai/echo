---
id: TC-020
title: A Transcript tab without a transcript says "Transcribing on this computer." while it is transcribed, else "No transcript for this call."
priority: medium
type: functional
requirements: [F-TR-04]
module: window
run: agent
platform: any
data: standard
tags: [transcript, empty-state]
---

# TC-020: A Transcript tab without a transcript says "Transcribing on this computer." while it is transcribed, else "No transcript for this call."

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie)
- A call's row is `[data-call$="-<slug>"]`; the tabs are `role=tab` "Minutes" / "Transcript"

## Test Data

| Call (today) | Status | Transcript tab |
|---|---|---|
| Quick check-in (`-quick-check-in`) | transcribing; kept live notes | the live layout: "The call ended", "Transcribing on this computer." beside "No live notes for this call." |
| Recording (`-recording`) | audio only, not transcribed | "No transcript for this call." |
| Umbrella intro call (`-umbrella-intro-call`) | transcription failed | "No transcript for this call." |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[data-call$="-quick-check-in"]`, then the tab "Transcript" | The Transcript tab is selected; the live layout's transcript column (`section#live-transcript`) reads "Transcribing on this computer."; the pane also reads "The call ended" and, beside it, "No live notes for this call."; no "Find in transcript" box |
| 2 | Click the row `[data-call$="-recording"]` (title "Recording"), tab "Transcript" | The pane reads "No transcript for this call."; no "Find in transcript" box |
| 3 | Click the row `[data-call$="-umbrella-intro-call"]`, tab "Transcript" | "No transcript for this call." |
| 4 | Click the row `[data-call$="-initech-planning"]`, tab "Transcript" | No empty text: the "Find in transcript" box and the turns (first "0:05" Me) |

## Expected Final State
Nothing changed.

## Notes
- The third text, while a call records ("Listening. The live transcript appears here as people speak." — "Listening. The transcript appears here as people speak." with live notes), needs a recording: covered with the test call (`window/TC-013`, person/test-call run).
- The feature's wording ("Transcribing on this computer…") is a paraphrase: the window says "Transcribing on this computer." in the live layout and "Transcribing on this computer. This takes about a quarter of the call length." without live notes (no such call in the data).
