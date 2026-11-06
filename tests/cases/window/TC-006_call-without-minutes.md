---
id: TC-006
title: A transcribed call without minutes offers to write them
priority: high
type: functional
requirements: [F-MIN-06]
module: window
run: agent
platform: any
tags: [calls, minutes]
external_id: TC-WIN-005
---

# TC-006: A transcribed call without minutes offers to write them

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Initech planning |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Initech planning" | The call opens on Minutes |
| 2 | Read the Minutes tab | "No minutes yet", "The transcript is ready", a line on which Claude Code model writes them, and a "Write minutes" button |
| 3 | Read under it | The command `/echo:process-call <the call's folder>` with "Copy" |
| 4 | Open the "Transcript" tab | Its two turns are there |

## Expected Final State
A call with a transcript and no minutes explains it, offers Write minutes and the session command; its transcript is readable. (Writing them needs Claude Code: not part of this case.)
