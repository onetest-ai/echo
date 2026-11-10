---
id: TC-002
title: The minutes list shows each template as Built in with what it is for
priority: medium
type: functional
requirements: [F-TPL-02]
module: templates
run: agent
platform: any
tags: [templates]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-002: The minutes list shows each template as Built in with what it is for

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` | The list "Minutes templates" shows four rows |
| 2 | Read the row "Default" | Tag "Built in"; "Any call no other template matches · 4 sections" |
| 3 | Read the row "Discussion" | Tag "Built in"; "discuss, review, design, architecture, planning, workshop, brainstorm · 5 sections" |
| 4 | Read the row "Status sync" | Tag "Built in"; "daily, sync, standup, stand-up, status, weekly · 4 sections" |
| 5 | Read the row "Memo" | Tag "Built in"; "Memos only — a part per thread · 2 sections" |

## Expected Final State
Four built-in templates, each with its use.
