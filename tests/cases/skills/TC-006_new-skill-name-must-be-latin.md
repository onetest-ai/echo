---
id: TC-006
title: New skill refuses a name without latin letters and says why in a toast
priority: low
type: functional
requirements: [F-SKILL-05, F-WS-06]
module: skills
run: agent
platform: any
tags: [skills, toast]
---

# TC-006: New skill refuses a name without latin letters and says why in a toast

**Module:** Skills | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Skills 1" and click "New skill" | An editor "New skill" opens with a box "Name" and the buttons "Cancel" and "Create and open" |
| 2 | Type `Звіт` in "Name" and click "Create and open" | A toast floats with the text "give the skill a name in latin letters (Claude Code's rule for skill names) — its text can be in any language" and a button "OK" |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/skills"` in Bash (`. <dir>/env` first) | No folder was made |
| 4 | Click "OK" on the toast | The toast goes away |
| 5 | Click "Cancel" | The editor shows "Choose a skill to see what Claude does with it." |

## Expected Final State
No skill was made; the refusal was shown as a toast.

## Notes
- Creating a valid skill opens its SKILL.md on this computer, so only the refusal is run here (the creation stays with the person).
