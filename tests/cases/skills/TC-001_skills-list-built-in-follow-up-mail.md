---
id: TC-001
title: Skills lists Echo's built-in skill with its switch and description
priority: medium
type: functional
requirements: [F-SKILL-01]
module: skills
run: agent
platform: any
tags: [skills]
---

# TC-001: Skills lists Echo's built-in skill with its switch and description

**Module:** Skills | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Skills 1" | A list "What Claude does with a task you send it" with one row "Follow up mail" |
| 2 | Read the row | Tag "Built in"; no "⚠" count; the description starts "Use when a task is to write a follow-up mail after a meeting" |
| 3 | Read the row's switch (checkbox "Follow up mail on") | It is checked; its title is "On — Claude can use it" |
| 4 | Read the buttons above the list | "Ask Claude" and "New skill" |

## Expected Final State
One built-in skill, on, with no problems.
