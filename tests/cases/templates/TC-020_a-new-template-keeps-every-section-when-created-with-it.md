---
id: TC-020
title: A new template starts without Keep every section, keeps it when created with it ticked, and through later edits
priority: medium
type: functional
requirements: [F-TPL-08]
module: templates
run: agent
platform: any
tags: [templates]
---

# TC-020: A new template starts without Keep every section, keeps it when created with it ticked, and through later edits

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/default.md`, `<data>/.echo/templates/board-review.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, click the row "Default" and tick "Keep every section" | The row "Default" reads "… sections, all kept" (Default itself keeps every section now) |
| 2 | Click "New template" | The new template's form shows "Keep every section" unticked (nothing is taken over from Default) |
| 3 | Type `Board review` as its name, tick "Keep every section" and click "Create" | A row "Board review" shows with the tag "Yours" and reads "… sections, all kept" |
| 4 | Reload the page and click the row "Board review" | "Keep every section" is ticked |
| 5 | Run `target/debug/echoctl templates` | The entry with `"id": "board-review"` has `"fixed": true` |
| 6 | Add the title word `board` (type it in the words box and press Enter); wait 1 s; run the command of step 5 again | "Keep every section" is still ticked; `board-review` still has `"fixed": true` and its `match` holds `board` |

## Expected Final State
Board review is the user's own template and keeps every section; Default keeps every section.

## Teardown
- Click "Delete" in the editor of "Board review".
- Open "Default" and click "Reset to built-in".
