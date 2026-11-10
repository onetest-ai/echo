---
id: TC-015
title: Changing a built-in meeting type marks it Edited
priority: medium
type: functional
requirements: [F-TPL-05]
module: templates
run: agent
platform: any
tags: [templates, meeting-types]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-015: Changing a built-in meeting type marks it Edited

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/meeting-types.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, click the tab "Meeting types 6" and click the row "Client" | The editor shows "Client", tag "Built in" and no "Reset to built-in" |
| 2 | Replace the text of "How to tell it" with `A client or prospect takes part: status, workshop, demo.` and press Tab | The row "Client" has the tag "Edited" and the new text; the editor shows "Reset to built-in" |
| 3 | Run `cat "$ECHOCTL_HOME/.echo/meeting-types.json"` in Bash (`. <dir>/env` first) | An entry with `"id": "client"` and the new sentence |

## Expected Final State
Client is Edited and saved.

## Teardown
- Click "Reset to built-in" in the editor of "Client".
