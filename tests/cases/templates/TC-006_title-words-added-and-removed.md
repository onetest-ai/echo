---
id: TC-006
title: Title words are added with Enter and removed with ×
priority: medium
type: functional
requirements: [F-TPL-03]
module: templates
run: agent
platform: any
tags: [templates]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-006: Title words are added with Enter and removed with ×

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/status-sync.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the row "Status sync" | The chips are daily, sync, standup, stand-up, status, weekly |
| 2 | Type `retro` in the box "Add a word" and press Enter | A seventh chip "retro" shows after "weekly"; the row "Status sync" reads "daily, sync, standup, stand-up, status, weekly, retro · 4 sections" and has the tag "Edited" |
| 3 | Run `grep -n '^match' "$ECHOCTL_HOME/.echo/templates"/status-sync.md` in Bash (`. <dir>/env` first) | Prints `match: [daily, sync, standup, stand-up, status, weekly, retro]` |
| 4 | Click the × of the chip "retro" (button "Remove retro") | The chip is gone; the row reads "daily, sync, standup, stand-up, status, weekly · 4 sections" |
| 5 | Run the grep again | Prints `match: [daily, sync, standup, stand-up, status, weekly]` |

## Expected Final State
The words are as they began, but the template is Edited.

## Teardown
- Click "Reset to built-in" in the editor of "Status sync".
