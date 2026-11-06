---
id: TC-005
title: Reset to built-in brings Echo's template back and keeps the edit in the trash
priority: high
type: functional
requirements: [F-TPL-04]
module: templates
run: agent
platform: any
tags: [templates]
---

# TC-005: Reset to built-in brings Echo's template back and keeps the edit in the trash

**Module:** Templates | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/`, `<data>/.echo/trash/templates/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, open "Status sync", replace the text of the box "What to write in Summary" with `Two lines on the week for Acme.` and press Tab | The row shows the tag "Edited" |
| 2 | Click "Reset to built-in" | The row "Status sync" shows the tag "Built in"; the editor shows no "Reset to built-in" button; the Summary box holds the built-in text, not `Two lines on the week for Acme.` |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/templates"` in Bash (`. <dir>/env` first) | The folder is empty |
| 4 | Run `ls "$ECHOCTL_HOME/.echo/trash/templates"` | One file named `status-sync-<digits>.md` |

## Expected Final State
Status sync is Echo's again; the user's edit sits in `.echo/trash/templates`.
