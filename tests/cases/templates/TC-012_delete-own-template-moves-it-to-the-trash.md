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
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-012: Delete moves the user's own template to the trash

**Module:** Templates | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** the template "Client kickoff" (Yours), its editor open. In S-11: made by stage 22 (templates/TC-011, its teardown skipped), so step 1 only counts N there. On its own: step 1 makes it.

**Changes:** `<data>/.echo/templates/`, `<data>/.echo/trash/templates/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `ls "$ECHOCTL_HOME/.echo/trash/templates" 2>/dev/null \| grep -c '^client-kickoff-[0-9]*\.md$'` in Bash (`. <dir>/env` first) and note the count as N (an earlier stage's teardown may have trashed one already); then navigate to `{{base_url}}/#settings/templates`, click "New template", type `Client kickoff` in "Name" and click "Create" | The row "Client kickoff" shows with the tag "Yours" |
| 2 | Click "Delete" in its editor | The row is gone; the tab reads "Minutes 4"; the editor shows "Choose a template to see and change it." |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/templates" 2>/dev/null` in Bash (`. <dir>/env` first) | Nothing is listed (the folder is empty or does not exist) |
| 4 | Run the same `ls "$ECHOCTL_HOME/.echo/trash/templates" 2>/dev/null \| grep -c '^client-kickoff-[0-9]*\.md$'` again | The count of files named `client-kickoff-<digits>.md` is N+1 (the new one is the file not listed before) |

## Expected Final State
The template is gone from the list and kept in `.echo/trash/templates`.
