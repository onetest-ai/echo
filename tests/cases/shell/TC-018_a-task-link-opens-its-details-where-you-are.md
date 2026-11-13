---
id: TC-018
title: A task link clicked in a prep opens the task's Details in the right panel; the prep stays open
priority: high
type: functional
requirements: [F-WS-11, F-WS-08]
module: shell
run: agent
platform: any
data: standard
tags: [shell, links, prep]
---

# TC-018: A task link clicked in a prep opens the task's Details in the right panel; the prep stays open

**Module:** Window shell | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie)
- Window at least 1400x900 (the right panel beside the page, not over it)

## Test Data

| Field | Value |
|---|---|
| Meeting | Globex sponsor update (to come), prep `e11de173ab857a8b` |
| Link | Open with these people → Task "Get the access list" (`echo://task/acme-rollout/T002`) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#upcoming/e11de173ab857a8b`, wait 2 s | The heading "Globex sponsor update" with its Prep tab; under OPEN WITH THESE PEOPLE, "Task" "Get the access list" · "Omar Haddad" · "late" |
| 2 | Click the link "Get the access list" | The hash stays `#upcoming/e11de173ab857a8b`; the heading is still "Globex sponsor update" with its prep; the right panel shows "Details" with "T002", "dependency", the title "Get the access list", "General › SSO for the Berlin pilot" and Status "to do" |
| 3 | Click the link "Send the SSO checklist" (LAST TIME → Still to do) | The hash stays `#upcoming/e11de173ab857a8b`; the right panel's Details now shows "T001" and the title "Send the SSO checklist" |
| 4 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Ftask%2Facme-rollout%2FT001` (a link handed over from outside Echo) | The hash becomes `#project/acme-rollout/work`; the Details show "T001" "Send the SSO checklist" (as `shell/TC-016` step 5) |

## Expected Final State
Nothing changed; only panels opened.

## Notes
- A task link inside a call's minutes or a Brain view takes the same route (`window.echo.openLink(link, true)`); the standard data has no minutes with a task link, so this case covers the prep.
- Inside Threads a task link still opens the thread's Work tab.
