---
id: TC-025
title: A run that wrote nothing says it failed or was skipped, with the reason and the right button
priority: high
type: functional
requirements: [F-MIN-09]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, run]
---

# TC-025: A run that wrote nothing says it failed or was skipped, with the reason and the right button

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Run | Heading | Text | Button |
|---|---|---|---|---|
| Acme security review | failed | Couldn't write the minutes | Claude Code exited with an error (exit 1) | Try again |
| Umbrella proposal walkthrough (`umbrella-proposal-walkthrough`) | skipped | No minutes for this call | less than 1,500 characters of transcript | Write minutes |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme security review, then the tab "Minutes" | No minutes frame; a box with the heading "Couldn't write the minutes" and the text "Claude Code exited with an error (exit 1)" |
| 2 | Read the box's line about the model and its buttons | "Claude Code writes them with Claude · work (sonnet), with your Claude account."; a select "Model for these minutes"; the button "Try again"; "Or in a Claude Code session:" `/echo:process-call <folder>` with a Copy button |
| 3 | Click the row Umbrella proposal walkthrough | A box with the heading "No minutes for this call" and the text "less than 1,500 characters of transcript" |
| 4 | Read its buttons | "Write minutes" (not "Try again"); the select "Model for these minutes"; the session line |

## Expected Final State
Umbrella proposal walkthrough shows why it has no minutes and offers Write minutes.

## Notes
- Creates or changes nothing (no button pressed). Run before `TC-024`, which changes the first message.
