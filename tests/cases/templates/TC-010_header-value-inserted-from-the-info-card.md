---
id: TC-010
title: The header's info card inserts a {value} into the header
priority: low
type: functional
requirements: [F-TPL-03]
module: templates
run: agent
platform: any
tags: [templates]
---

# TC-010: The header's info card inserts a {value} into the header

**Module:** Templates | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/status-sync.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the row "Status sync" | The editor shows "Header — how the minutes start" |
| 2 | Click the button "Values and how a header is read" | A note lists the buttons {title}, {date}, {time}, {duration}, {participants}, {colleagues}, {external}, {organizer}, {type}, {thread}, {client}, {previous}, {me}, {language} |
| 3 | Click the button {language} | The header box now ends with `{language}`; the row has the tag "Edited" |
| 4 | Run `grep -c '{language}' "$ECHOCTL_HOME/.echo/templates"/status-sync.md` in Bash (`. <dir>/env` first) | Prints `1` |

## Expected Final State
The header holds {language} and the template is Edited.

## Teardown
- Click "Reset to built-in" in the editor of "Status sync".
