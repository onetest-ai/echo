---
id: TC-019
title: A call that kept live notes shows them beside its transcript; one without shows none
priority: medium
type: functional
requirements: [F-CALLS-07]
module: calls
run: agent
platform: any
data: standard
tags: [calls, live-notes, transcript]
---

# TC-019: A call that kept live notes shows them beside its transcript; one without shows none

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Live notes |
|---|---|
| Acme weekly sync (Today) | Now: Berlin pilot go-ahead · Decisions: The pilot goes ahead in Berlin (4:10) · Action items: Rita Gomez Finish the dry run (0:45) · Open questions: Who replaces Yuki? (2:31) · Not covered: 10:00–11:00 · asked: What did Omar say about the questionnaire? → It is on hold until legal reviews the data processing terms. |
| Initech planning | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync, then the tab "Transcript" | A region `aria-label="Live notes"` is beside the transcript region (`aria-label="Transcript"`); the state line says "The call ended" |
| 2 | Read the live notes | "Now" "Berlin pilot go-ahead"; headings Decisions, Action items, Open questions with "The pilot goes ahead in Berlin", "Rita Gomez" "Finish the dry run", "Who replaces Yuki?"; "Not covered: 10:00–11:00" |
| 3 | Read the answers under the notes | The question "What did Omar say about the questionnaire?" with the answer "It is on hold until legal reviews the data processing terms."; the button "Ask Claude about this call" is there (do not press it) |
| 4 | Read the transcript region | Five turns of named speakers; the `#find` box is in the live header |
| 5 | Click the row Initech planning | The Transcript tab shows its turns and the Find box, with no region "Live notes" |

## Expected Final State
Initech planning's Transcript tab is open with no live notes.

## Notes
- Creates or changes nothing. Live notes while a call runs (`live.state`) need the app recording: skipped.
