---
id: TC-023
title: A failed run over existing minutes shows a bar above them; a finished run shows none
priority: high
type: functional
requirements: [F-MIN-09]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, run]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-023: A failed run over existing minutes shows a bar above them; a finished run shows none

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Run | Above the minutes |
|---|---|---|
| Acme weekly sync (Today) | done | no bar |
| Globex pilot kickoff | failed, had minutes | "The last update didn't finish: Claude Code exited with an error (exit 1)" · Try again (no Show log) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync, then the tab "Minutes" | The minutes frame is shown; nothing above it (no "didn't finish", no "Updating", no "waits its turn") |
| 2 | Click the row Globex pilot kickoff | The minutes frame is shown AND a bar above it reads "The last update didn't finish: Claude Code exited with an error (exit 1)" |
| 3 | Read the bar's buttons | "Try again" is there; "Show log" is not (the call has no log); there is no "Sign in…" |

## Expected Final State
Globex pilot kickoff shows its minutes under a failure bar.

## Notes
- Creates or changes nothing. Do this before `TC-024`, which changes the message.
