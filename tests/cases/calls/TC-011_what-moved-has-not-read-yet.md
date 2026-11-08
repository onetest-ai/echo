---
id: TC-011
title: What moved says a thread has not read the call yet and offers Sync now
priority: medium
type: functional
requirements: [F-MIN-04, F-MIN-05]
module: calls
run: agent
platform: any
tags: [minutes, what-moved]
---

# TC-011: What moved says a thread has not read the call yet and offers Sync now

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Run at least 15 minutes after the call ended (the data puts Acme weekly sync 3 h back, but not before 00:05 today): after that the line says "within a few minutes".

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync |
| Thread | Acme rollout |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync", then open What moved with the header button `[data-panel-flip="moved"]` (beside My notes; skip the click if the right panel already shows What moved) | The call opens on Minutes, the minutes at the tab's full width; the right panel (`#r-right`) shows the view "What moved" |
| 2 | Read the What moved view (`#r-right .mv`) | The panel title "What moved"; the thread "Acme rollout"; the text "reads this call within a few minutes"; no count on the tab or the button (the thread hasn't read the call) |
| 3 | Look for the button "Sync now" (`[data-cc-sync="acme-rollout"]`) | It is there; do not press it (it starts Claude Code) |
| 4 | Open "Initech planning" | No What moved button in the header and no What moved tab in the panel (the call is in no thread) |

## Expected Final State
The aside of Acme weekly sync names Acme rollout and offers Sync now; Initech planning has none.

## Notes
- Creates or changes nothing. The changes list (new task, done, due, weak, kept yours) needs a synced thread: `standard` data set.
