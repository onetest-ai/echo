---
id: TC-019
title: A call that kept live notes shows the transcript alone in its Transcript tab; the live notes are in My notes
priority: medium
type: functional
requirements: [F-CALLS-07]
module: calls
run: agent
platform: any
data: standard
tags: [calls, live-notes, transcript]
---

# TC-019: A call that kept live notes shows the transcript alone in its Transcript tab; the live notes are in My notes

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Live notes |
|---|---|
| Acme weekly sync (Today) | Now: Berlin pilot go-ahead · Decisions: The pilot goes ahead in Berlin (4:10) · Action items: Rita Gomez Finish the dry run (0:45) · Open questions: Who replaces Yuki? (2:31) · Not covered: 10:00–11:00 · asked: What did Omar say about the questionnaire? → It is on hold until legal reviews the data processing terms. |
| Initech planning | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync, then the tab "Transcript" | The pane shows the "Find in transcript" box (`#find`) and the turns of named speakers (17), at the pane's full width; there is no region `aria-label="Live notes"` in the pane (`#pane`) and no "Ask Claude about this call" button in it |
| 2 | Open My notes (`[data-panel-flip="notes"]`, unless it is open) | The panel `#r-right` has the region `section[aria-label="Live notes"]` with "Now" "Berlin pilot go-ahead" and the state "The call ended"; its list holds ✦ rows "Finish the dry run" (Rita Gomez), "Who replaces Yuki?" and "The pilot goes ahead in Berlin"; under "Asked about this call" the question "What did Omar say about the questionnaire?" with "It is on hold until legal reviews the data processing terms." |
| 3 | Click the row Initech planning | The Transcript tab shows its turns and the Find box; My notes has no region "Live notes" and no ✦ row |

## Expected Final State
Initech planning's Transcript tab is open; My notes shows no live notes.

## Notes
- Creates or changes nothing. Live notes while a call runs (`live.state`) need the app recording: skipped. The live notes' timeline in My notes is window/TC-023.
