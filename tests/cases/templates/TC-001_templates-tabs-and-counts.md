---
id: TC-001
title: Templates has the tabs Minutes, Meeting types and Skills with their counts
priority: medium
type: functional
requirements: [F-TPL-01]
module: templates
run: agent
platform: any
tags: [templates]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-001: Templates has the tabs Minutes, Meeting types and Skills with their counts

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` | The heading "Templates" and the sentence "The shapes Claude writes in." show |
| 2 | Read the tabs under "Kind of template" | "Minutes 4", "Meeting types 6", "Skills 1"; "Minutes" is selected |
| 3 | Click the tab "Meeting types 6" | It is selected; the list is headed "What a call can be, and how to tell"; a button "New type" shows |
| 4 | Click the tab "Skills 1" | It is selected; a button "New skill" shows |
| 5 | Click the tab "Minutes 4" | It is selected; a button "New template" shows |

## Expected Final State
The three tabs switch the list below them.
