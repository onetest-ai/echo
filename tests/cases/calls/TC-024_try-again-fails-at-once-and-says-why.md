---
id: TC-024
title: Try again ends in a failure that names the fake Claude Code, with minutes or without
priority: high
type: functional
requirements: [F-MIN-09, F-MIN-13]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, run, fake-claude]
---

# TC-024: Try again ends in a failure that names the fake Claude Code, with minutes or without

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)
- The data's `claude` is a fake that refuses every run (`tests/fixtures/README.md`): no Claude Code account is used

## Test Data

| Call | Before | After Try again |
|---|---|---|
| Globex pilot kickoff | bar: "…Claude Code exited with an error (exit 1)" | bar: "The last update didn't finish: test data: Claude Code is a fake here"; Show log appears |
| Acme security review (`acme-security-review`) | box: "Couldn't write the minutes", "Claude Code exited with an error (exit 1)" | same heading, text "test data: Claude Code is a fake here"; Show log appears |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Globex pilot kickoff, then the tab "Minutes" | The bar shows "Claude Code exited with an error (exit 1)" with "Try again" |
| 2 | Click "Try again" and wait up to 15 seconds for the text "test data: Claude Code is a fake here" | The bar reads "The last update didn't finish: test data: Claude Code is a fake here"; the minutes frame is still there; "Try again" and "Show log" are shown |
| 3 | Click the row Acme security review | The box heading is "Couldn't write the minutes" with the text "Claude Code exited with an error (exit 1)" and "Try again" |
| 4 | Click "Try again" and wait up to 15 seconds | The heading stays "Couldn't write the minutes"; the text now reads "test data: Claude Code is a fake here"; "Show log" is shown (do not press it: it opens a file) |
| 5 | Open the Inbox panel (button "Inbox") and find "Minutes failed: Acme security review" | Its line reads "test data: Claude Code is a fake here"; the buttons "Write again" and "Show log" are still there |

## Expected Final State
Both calls show the fake's message; nothing is running.

## Teardown
- Click "Inbox" to close the panel
- The run state stays failed; cases that read the first messages (`TC-023`, `TC-025`) run before this one or on fresh data

## Notes
- Step 5 is the check for #121 (the Inbox kept the old "Claude Code exited with an error (exit 1)" after Try again,
  until a full reload).
- Verified against the code and a served window: `write_notes` starts a run, the fake exits at once and the run ends `failed` with that message.
