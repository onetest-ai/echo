---
id: TC-004
title: Customize copies a built-in skill to the user's folder
priority: medium
type: functional
requirements: [F-SKILL-03]
module: skills
run: agent
platform: any
tags: [skills]
---

# TC-004: Customize copies a built-in skill to the user's folder

**Module:** Skills | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/skills/follow-up-mail/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Skills 1" and click the row "Follow up mail" | The editor shows the button "Customize" |
| 2 | Click "Customize" | The row's tag turns "Customized"; the editor shows the buttons "Open in VS Code", "Show in Finder" and "Reset" (none of the first two is pressed) |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/skills/follow-up-mail"` in Bash (`. <dir>/env` first) | Prints `SKILL.md` |

## Expected Final State
The skill is Customized and its file is in the data folder.

## Teardown
- Click "Reset" in the editor of "Follow up mail".

## Notes
- Echo also opens the new SKILL.md in this computer's default app (`open`); close that window. Nothing else leaves the run.
