---
id: TC-012
title: Delete moves the user's own template to the trash
priority: high
type: functional
requirements: [F-TPL-04]
module: templates
run: agent
platform: any
tags: [templates]
---

# TC-012: Delete moves the user's own template to the trash

**Module:** Templates | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/`, `<data>/.echo/trash/templates/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, click "New template", type `Client kickoff` in "Name" and click "Create" | The row "Client kickoff" shows with the tag "Yours" |
| 2 | Click "Delete" in its editor | The row is gone; the tab reads "Minutes 4"; the editor shows "Choose a template to see and change it." |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/templates"` in Bash (`. <dir>/env` first) | The folder is empty |
| 4 | Run `ls "$ECHOCTL_HOME/.echo/trash/templates"` | One file named `client-kickoff-<digits>.md` |

## Expected Final State
The template is gone from the list and kept in `.echo/trash/templates`.
