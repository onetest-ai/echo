---
id: TC-016
title: Reset to built-in brings Echo's meeting type back
priority: medium
type: functional
requirements: [F-TPL-05]
module: templates
run: agent
platform: any
tags: [templates, meeting-types]
---

# TC-016: Reset to built-in brings Echo's meeting type back

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/meeting-types.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, click the tab "Meeting types 6", open "Client", replace "How to tell it" with `A client or prospect takes part: status, workshop, demo.` and press Tab | The row has the tag "Edited" |
| 2 | Click "Reset to built-in" | The row "Client" has the tag "Built in" and the text "A client's or prospect's people take part, about their work: a status meeting, a workshop, a demo, a negotiation." |
| 3 | Run `cat "$ECHOCTL_HOME/.echo/meeting-types.json"` in Bash (`. <dir>/env` first) | The file holds `[]` |

## Expected Final State
Client is Echo's own again.
