---
id: TC-002
title: Opening a skill shows its SKILL.md and how Claude names it
priority: medium
type: functional
requirements: [F-SKILL-02]
module: skills
run: agent
platform: any
tags: [skills]
---

# TC-002: Opening a skill shows its SKILL.md and how Claude names it

**Module:** Skills | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Skills 1" and click the row "Follow up mail" | The editor titled "Follow up mail" with the tag "Built in" opens |
| 2 | Read the line above the text | "SKILL.md · in Claude as /my-skills:follow-up-mail" |
| 3 | Read the text | It starts with `---`, then `name: follow-up-mail` and a `description:` line, then `# Follow-up mail` |
| 4 | Look for "To fix before Claude can use it" | Absent |
| 5 | Read the editor's buttons | "Customize" only; no "Open in VS Code" (Echo's own skill has no folder yet) |

## Expected Final State
The skill's file is shown as written.
