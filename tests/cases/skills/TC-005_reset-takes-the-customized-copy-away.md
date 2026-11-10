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
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-005: Reset takes the customized copy away and keeps it in the trash

**Module:** Skills | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** "Follow up mail" Customized, its editor open. In S-12: made by stage 4 (skills/TC-004, its teardown skipped), so step 1 is skipped there and N is 0. On its own: step 1 makes it.

**Changes:** `<data>/.echo/skills/`, `<data>/.echo/trash/skills/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Skills 1", click the row "Follow up mail" and click "Customize" | The tag is "Customized" |
| 2 | Run `ls "$ECHOCTL_HOME/.echo/trash/skills" 2>/dev/null \| grep -c '^follow-up-mail-'` in Bash (`. <dir>/env` first) and keep the number as N | A number: 0 on fresh data, more when an earlier stage already reset this skill |
| 3 | Click "Reset" | The row's tag is "Built in" again; the editor shows only "Customize" |
| 4 | Run `ls "$ECHOCTL_HOME/.echo/skills"` | Prints nothing (no `follow-up-mail` folder) |
| 5 | Run `ls "$ECHOCTL_HOME/.echo/trash/skills" \| grep '^follow-up-mail-'` | N + 1 folders named `follow-up-mail-<digits>`: the Reset added one |

## Expected Final State
The skill is Echo's own again; the copy sits in `.echo/trash/skills`.

## Notes
- Customize opens SKILL.md in this computer's default app; close it.
