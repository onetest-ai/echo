---
id: TC-030
title: A call held by Not yet or too thin for automatic minutes says so and still offers Write minutes
priority: high
type: functional
requirements: [F-MIN-08]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, auto]
---

# TC-030: A call held by Not yet or too thin for automatic minutes says so and still offers Write minutes

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | auto.due | Heading | Text | Extra button |
|---|---|---|---|---|
| Globex support triage (`globex-support-triage`) | held | Minutes when you say | You asked for these later: Echo won't write them by itself. | Write them automatically again |
| Coffee chat (`coffee-chat`) | gate | No minutes written automatically | The call is too thin for them (shorter than 3 minutes). Write them anyway if you want them. | none |
| Globex weekly (`globex-weekly`, imported) | gate | No minutes written automatically | The call is too thin for them (less than 1,500 characters of transcript). Write them anyway if you want them. | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Globex support triage, then the tab "Minutes" | The heading "Minutes when you say" and the text "You asked for these later: Echo won't write them by itself." |
| 2 | Read its buttons | "Write them automatically again" (do not press it) and "Write minutes" |
| 3 | Click the row Coffee chat | The heading "No minutes written automatically" and the text "The call is too thin for them (shorter than 3 minutes). Write them anyway if you want them." |
| 4 | Read its buttons | "Write minutes"; no "Write them automatically again" |
| 5 | Click the row Globex weekly | The heading "No minutes written automatically" and the text with "(less than 1,500 characters of transcript)"; the button "Write minutes" |

## Expected Final State
Globex weekly shows why nothing is written by itself.

## Notes
- Creates or changes nothing. Do not press Write minutes (starts Claude Code).
