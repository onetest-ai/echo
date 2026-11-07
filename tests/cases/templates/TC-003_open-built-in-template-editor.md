---
id: TC-003
title: A built-in template opens in an editor with its words and sections
priority: medium
type: functional
requirements: [F-TPL-03]
module: templates
run: agent
platform: any
tags: [templates]
---

# TC-003: A built-in template opens in an editor with its words and sections

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the row "Status sync" | The editor opens on the right: title "Status sync", tag "Built in", and no button "Reset to built-in" |
| 2 | Read "Used when the title has" | Chips: daily, sync, standup, stand-up, status, weekly |
| 3 | Read the headings of the sections | In order: "Summary", "Progress", "Blockers & risks", "Action items" |
| 4 | Read the editor's foot | "Saved as you change it · the next minutes follow it" |
| 5 | Click the close button ("Close") of the editor | The editor closes and shows "Choose a template to see and change it." |

## Expected Final State
The built-in template was only read; nothing changed.
