---
id: TC-004
title: Changing a built-in template saves at once and marks it Edited
priority: high
type: functional
requirements: [F-TPL-03, F-TPL-04]
module: templates
run: agent
platform: any
tags: [templates]
---

# TC-004: Changing a built-in template saves at once and marks it Edited

**Module:** Templates | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/status-sync.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, click the row "Status sync"; run `ls "$ECHOCTL_HOME/.echo/templates" 2>/dev/null` in Bash (`. <dir>/env` first) | The editor shows the tag "Built in"; nothing is listed (the folder does not exist yet or is empty) |
| 2 | Replace the text of the first section's box ("What to write in Summary") with `Two lines on the week for Acme.` and press Tab | The box holds that text |
| 3 | Wait 1 s and read the row "Status sync" and the editor's title bar | The row shows the tag "Edited"; the editor shows "Edited" and a button "Reset to built-in" |
| 4 | Run `ls "$ECHOCTL_HOME/.echo/templates"` and `grep -n 'Two lines on the week' "$ECHOCTL_HOME/.echo/templates"/status-sync.md` | The folder holds `status-sync.md`; the grep prints one line with `Two lines on the week for Acme.` |
| 5 | Reload the page and open "Status sync" again | The tag is still "Edited" and the Summary box holds `Two lines on the week for Acme.` |

## Expected Final State
Status sync is Edited and its file is in the data folder.

## Teardown
- Click "Reset to built-in" in the editor of "Status sync".
