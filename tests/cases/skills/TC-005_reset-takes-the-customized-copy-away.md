---
id: TC-005
title: Reset takes the customized copy away and keeps it in the trash
priority: medium
type: functional
requirements: [F-SKILL-03]
module: skills
run: agent
platform: any
tags: [skills]
---

# TC-005: Reset takes the customized copy away and keeps it in the trash

**Module:** Skills | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/skills/`, `<data>/.echo/trash/skills/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Skills 1", click the row "Follow up mail" and click "Customize" | The tag is "Customized" |
| 2 | Click "Reset" | The row's tag is "Built in" again; the editor shows only "Customize" |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/skills"` in Bash (`. <dir>/env` first) | Prints nothing (no `follow-up-mail` folder) |
| 4 | Run `ls "$ECHOCTL_HOME/.echo/trash/skills"` | One folder named `follow-up-mail-<digits>` |

## Expected Final State
The skill is Echo's own again; the copy sits in `.echo/trash/skills`.

## Notes
- Customize opens SKILL.md in this computer's default app; close it.
