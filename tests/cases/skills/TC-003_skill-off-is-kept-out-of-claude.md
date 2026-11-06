---
id: TC-003
title: A skill switched off is recorded as off and switched on again
priority: high
type: functional
requirements: [F-SKILL-01, F-SKILL-04]
module: skills
run: agent
platform: any
tags: [skills]
---

# TC-003: A skill switched off is recorded as off and switched on again

**Module:** Skills | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/skills-off.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Skills 1"; run `cat "$ECHOCTL_HOME/.echo/skills-off.json"` in Bash (`. <dir>/env` first) if the file exists | No file, or `[]` |
| 2 | Uncheck the checkbox "Follow up mail on" | The row looks dimmed; the switch's title is "Off — kept out of Claude" |
| 3 | Wait 1 s, run `cat "$ECHOCTL_HOME/.echo/skills-off.json"` | The file holds `["follow-up-mail"]` |
| 4 | Check the checkbox "Follow up mail on" | The title is "On — Claude can use it" |
| 5 | Run `cat "$ECHOCTL_HOME/.echo/skills-off.json"` | The file holds `[]` |

## Expected Final State
The skill is on, the off-list empty.

## Teardown
- Check the checkbox "Follow up mail on" if it is unchecked.

## Notes
- The other half of F-SKILL-04 (the skill missing from the Send to Claude dialog's list) needs a task and the dialog; it is not covered here.
