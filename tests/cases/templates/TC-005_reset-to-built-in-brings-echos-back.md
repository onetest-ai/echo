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
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-005: Reset to built-in brings Echo's template back and keeps the edit in the trash

**Module:** Templates | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** Status sync Edited, with `Two lines on the week for Acme.` in its Summary box and its editor open. In S-11: made by stage 15 (templates/TC-004, its teardown skipped), so step 1 is skipped there. On its own: step 1 makes it.

**Changes:** `<data>/.echo/templates/`, `<data>/.echo/trash/templates/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, open "Status sync", replace the text of the box "What to write in Summary" with `Two lines on the week for Acme.` and press Tab | The row shows the tag "Edited" |
| 2 | Click "Reset to built-in" | The row "Status sync" shows the tag "Built in"; the editor shows no "Reset to built-in" button; the Summary box holds the built-in text, not `Two lines on the week for Acme.` |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/templates" 2>/dev/null` in Bash (`. <dir>/env` first) | Nothing is listed (the folder is empty or does not exist) |
| 4 | Run `ls "$ECHOCTL_HOME/.echo/trash/templates" | grep -c "^status-sync-[0-9]*\.md$"` | One more than before this case (an earlier case may already have put one there) |

## Expected Final State
Status sync is Echo's again; the user's edit sits in `.echo/trash/templates`.
