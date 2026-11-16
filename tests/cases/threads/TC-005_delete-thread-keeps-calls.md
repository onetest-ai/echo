---
id: TC-005
title: Thread actions → Delete… asks first; Move to trash removes the thread and leaves its calls
priority: high
type: functional
requirements: [F-THR-06]
module: threads
run: agent
platform: any
tags: [threads, delete]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-005: Thread actions → Delete… asks first; Move to trash removes the thread and leaves its calls

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread: `echoctl project create "Scratch thread"`, then put the call in it: `echoctl project include scratch-thread <folder of Initech planning>` (the folder is the entry of `$ECHOCTL_HOME/calls` ending `-initech-planning`)

## Test Data

| Field | Value |
|-------|-------|
| Thread | Scratch thread (`scratch-thread`) |
| Call in it | Initech planning |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/scratch-thread/calls` | The heading "Scratch thread"; the tab "Calls" has count 1; a row "Initech planning" |
| 2 | Click "Thread actions" then "Delete…" | A notice reads "Delete Scratch thread? Its brief, profile and work go to the trash; its calls stay." with buttons "Cancel" and "Move to trash" |
| 3 | Click "Cancel" | The notice disappears; the heading "Scratch thread" is still shown |
| 4 | Click "Thread actions", "Delete…", then "Move to trash". Wait up to 3 s after an action for the window to redraw before reading. | The URL ends with `#projects`; the left list has no row "Scratch thread" |
| 5 | Run `echoctl project list` | The JSON `projects` has no id `scratch-thread` |
| 6 | Navigate to `{{base_url}}/#calls` | The call "Initech planning" is still listed |

## Expected Final State
The thread is gone from the list and from `project list`; the call Initech planning still exists.

## Teardown
- None: the thread is deleted by the case. If it failed before step 4, run `echoctl project delete scratch-thread`

## Changes
- Creates and deletes thread `scratch-thread`.
