---
id: TC-013
title: Meeting types lists Echo's six, each with how to tell it
priority: medium
type: functional
requirements: [F-TPL-05]
module: templates
run: agent
platform: any
tags: [templates, meeting-types]
---

# TC-013: Meeting types lists Echo's six, each with how to tell it

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the tab "Meeting types 6" | Six rows, each with the tag "Built in" |
| 2 | Read the names in order | Client, Partner, 1:1, Internal, Industry & Analyst, Other |
| 3 | Read the line under "Client" | "A client's or prospect's people take part, about their work: a status meeting, a workshop, a demo, a negotiation." |
| 4 | Read the line under "1:1" | "The user and one colleague, nobody else." |

## Expected Final State
Echo's six meeting types are listed with their descriptions.
